/// Image generation via Replicate API — generates pixel art and writes to canvas.
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use pxlot_core::history::Command;

use crate::ai::ChatMessage;
use crate::state::EditorState;

use serde::Deserialize;

/// API endpoint for image generation (proxied through our server).
const GENERATE_IMAGE_URL: &str = "/api/generate-image";

/// Response from the server's /api/generate-image endpoint.
#[derive(Deserialize)]
struct GenerateImageResponse {
    image_base64: String,
    width: u32,
    height: u32,
}

/// Run image generation: call Replicate API via server proxy,
/// decode the returned PNG, and write pixels to the canvas.
pub async fn run_image_generation(
    user_text: String,
    editor: StoredValue<EditorState>,
    set_messages: WriteSignal<Vec<ChatMessage>>,
    set_running: WriteSignal<bool>,
    set_render_trigger: WriteSignal<u32>,
) {
    set_running.set(true);

    set_messages.update(|msgs| {
        msgs.push(ChatMessage::status("Generating pixel art image..."));
    });

    // Build the prompt — enhance it for pixel art
    let prompt = format!(
        "pixel art sprite, 64x64, side view, clean lines, no background, transparent background, {}",
        user_text
    );

    let request_body = serde_json::json!({
        "prompt": prompt,
        "model": "retro-diffusion/rd-plus",
        "width": 256,
        "height": 256,
    });

    // Call our server endpoint
    let response = match send_generate_request(&request_body).await {
        Ok(r) => r,
        Err(e) => {
            set_messages.update(|msgs| {
                msgs.push(ChatMessage::status(&format!("Image generation error: {e}")));
            });
            set_running.set(false);
            return;
        }
    };

    set_messages.update(|msgs| {
        msgs.push(ChatMessage::status("Image received, writing to canvas..."));
    });

    // Decode base64 PNG → RGBA pixels
    let pixels = match decode_png_base64(&response.image_base64) {
        Ok(p) => p,
        Err(e) => {
            set_messages.update(|msgs| {
                msgs.push(ChatMessage::status(&format!("Image decode error: {e}")));
            });
            set_running.set(false);
            return;
        }
    };

    // Write pixels to canvas (resize from generated size to canvas size)
    let pixel_count = write_pixels_to_canvas(
        &pixels,
        response.width,
        response.height,
        &editor,
    );

    set_render_trigger.update(|v| *v += 1);

    set_messages.update(|msgs| {
        msgs.push(ChatMessage::assistant(&format!(
            "Image generated and applied — {} pixels written.",
            pixel_count
        )));
    });

    set_running.set(false);
}

/// Send the generate-image request to our server proxy.
async fn send_generate_request(
    body: &serde_json::Value,
) -> Result<GenerateImageResponse, String> {
    let body_str =
        serde_json::to_string(body).map_err(|e| format!("Serialize error: {e}"))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body_str));

    let headers =
        web_sys::Headers::new().map_err(|e| format!("Headers error: {e:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("Header error: {e:?}"))?;

    // Attach JWT auth token
    if let Some(token) = crate::auth::load_token() {
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(|e| format!("Auth header error: {e:?}"))?;
    }
    opts.set_headers(&headers);

    let request =
        web_sys::Request::new_with_str_and_init(GENERATE_IMAGE_URL, &opts)
            .map_err(|e| format!("Request error: {e:?}"))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {e:?}"))?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| "Response cast error".to_string())?;

    let text_promise = resp.text().map_err(|e| format!("Text error: {e:?}"))?;
    let text_val = JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("Text read error: {e:?}"))?;
    let text = text_val
        .as_string()
        .ok_or_else(|| "Response not string".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP {}: {}", resp.status(), text));
    }

    serde_json::from_str::<GenerateImageResponse>(&text)
        .map_err(|e| format!("Parse error: {e}"))
}

/// Decoded RGBA pixel data from a PNG image.
struct DecodedImage {
    width: u32,
    height: u32,
    /// RGBA pixels, row-major, 4 bytes per pixel.
    data: Vec<u8>,
}

/// Decode a base64-encoded PNG into RGBA pixel data.
fn decode_png_base64(b64: &str) -> Result<DecodedImage, String> {
    use base64::Engine;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64 decode error: {e}"))?;

    // Use the png crate to decode
    let decoder = png::Decoder::new(bytes.as_slice());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode error: {e}"))?;

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG frame error: {e}"))?;

    let width = info.width;
    let height = info.height;

    // Convert to RGBA if needed
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in rgb.chunks(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255); // alpha
            }
            rgba
        }
        png::ColorType::GrayscaleAlpha => {
            let ga = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in ga.chunks(2) {
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let g = &buf[..info.buffer_size()];
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &v in g {
                rgba.push(v);
                rgba.push(v);
                rgba.push(v);
                rgba.push(255);
            }
            rgba
        }
        png::ColorType::Indexed => {
            return Err("Indexed PNG not supported, use RGB/RGBA".to_string());
        }
    };

    Ok(DecodedImage {
        width,
        height,
        data: rgba,
    })
}

/// Write decoded image pixels to the canvas, resizing with nearest-neighbor
/// to fit the canvas frame dimensions. Returns the number of pixels written.
fn write_pixels_to_canvas(
    image: &DecodedImage,
    _gen_width: u32,
    _gen_height: u32,
    editor: &StoredValue<EditorState>,
) -> u32 {
    editor.with_value(|state| {
        let fw = state.canvas.frame_width();
        let fh = state.canvas.frame_height();
        fw * fh // just for return value
    });

    let mut pixel_count = 0u32;

    editor.update_value(|state| {
        let fw = state.canvas.frame_width();
        let fh = state.canvas.frame_height();
        let fx = state.canvas.frame_x;
        let fy = state.canvas.frame_y;
        let src_w = image.width;
        let src_h = image.height;

        let mut cmd = Command::new("ai:image_generation".to_string());

        // Nearest-neighbor resize from source to canvas frame
        for dy in 0..fh {
            for dx in 0..fw {
                // Map canvas pixel to source pixel
                let sx = (dx as f64 * src_w as f64 / fw as f64) as u32;
                let sy = (dy as f64 * src_h as f64 / fh as f64) as u32;
                let sx = sx.min(src_w - 1);
                let sy = sy.min(src_h - 1);

                let idx = ((sy * src_w + sx) * 4) as usize;
                if idx + 3 >= image.data.len() {
                    continue;
                }

                let r = image.data[idx];
                let g = image.data[idx + 1];
                let b = image.data[idx + 2];
                let a = image.data[idx + 3];

                // Skip fully transparent pixels
                if a < 10 {
                    continue;
                }

                let color = pxlot_core::Color::new(r, g, b, a);
                let bx = fx + dx;
                let by = fy + dy;

                if let Some(layer) = state.canvas.layers.get_mut(state.canvas.active_layer) {
                    let old = layer.buffer.get_pixel(bx, by).copied()
                        .unwrap_or(pxlot_core::Color::TRANSPARENT);
                    layer.buffer.set_pixel(bx, by, color);
                    cmd.add_change(
                        state.canvas.active_layer,
                        bx,
                        by,
                        old,
                        color,
                    );
                }

                pixel_count += 1;
            }
        }

        if !cmd.is_empty() {
            state.history.push(cmd);
        }
    });

    pixel_count
}
