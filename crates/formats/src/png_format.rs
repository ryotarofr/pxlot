use pxlot_core::{Canvas, Color, Layer};
use std::io::Cursor;

/// Export the flattened canvas to PNG bytes at 1x scale.
pub fn export_png(canvas: &Canvas) -> Result<Vec<u8>, String> {
    export_png_scaled(canvas, 1)
}

/// Export the flattened canvas to PNG bytes, scaled up by `scale` using
/// nearest-neighbor interpolation. This keeps pixel art crisp at larger sizes.
/// Only the frame region is exported.
pub fn export_png_scaled(canvas: &Canvas, scale: u32) -> Result<Vec<u8>, String> {
    let scale = scale.max(1);
    let w = canvas.frame_width();
    let h = canvas.frame_height();
    let flat = canvas.flatten_frame_visible();

    let out_w = w * scale;
    let out_h = h * scale;

    let scaled = if scale == 1 {
        flat
    } else {
        let mut out = vec![0u8; (out_w * out_h * 4) as usize];
        for y in 0..out_h {
            let src_y = y / scale;
            for x in 0..out_w {
                let src_x = x / scale;
                let si = ((src_y * w + src_x) * 4) as usize;
                let di = ((y * out_w + x) * 4) as usize;
                out[di] = flat[si];
                out[di + 1] = flat[si + 1];
                out[di + 2] = flat[si + 2];
                out[di + 3] = flat[si + 3];
            }
        }
        out
    };

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut buf), out_w, out_h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&scaled).map_err(|e| e.to_string())?;
    }
    Ok(buf)
}

/// Export a single layer to PNG bytes.
pub fn export_layer_png(layer: &Layer) -> Result<Vec<u8>, String> {
    let w = layer.buffer.width;
    let h = layer.buffer.height;
    let rgba = layer.buffer.as_rgba_bytes();

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut buf), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    }
    Ok(buf)
}

/// Maximum dimension for imported images. Images larger than this will be
/// downscaled to fit while preserving aspect ratio.
pub const MAX_IMPORT_DIMENSION: u32 = 256;

/// Import a PNG from bytes into a new Canvas.
/// Images exceeding `MAX_IMPORT_DIMENSION` in either dimension are
/// downscaled using nearest-neighbor sampling.
pub fn import_png(data: &[u8]) -> Result<Canvas, String> {
    import_png_with_limit(data, MAX_IMPORT_DIMENSION)
}

/// Import a PNG with a custom max dimension limit.
pub fn import_png_with_limit(data: &[u8], max_dim: u32) -> Result<Canvas, String> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;

    let mut img_data = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut img_data).map_err(|e| e.to_string())?;

    let src_w = info.width;
    let src_h = info.height;

    // Decode source pixels into a flat RGBA buffer first
    let src_rgba = decode_to_rgba(&img_data, src_w, src_h, info.color_type)?;

    // Determine output dimensions (downscale if needed)
    let (w, h) = if src_w > max_dim || src_h > max_dim {
        let scale = (max_dim as f64) / (src_w.max(src_h) as f64);
        let new_w = ((src_w as f64 * scale) as u32).max(1);
        let new_h = ((src_h as f64 * scale) as u32).max(1);
        (new_w, new_h)
    } else {
        (src_w, src_h)
    };

    let mut canvas = Canvas::new(w, h);
    let fx0 = canvas.frame_x;
    let fy0 = canvas.frame_y;
    let layer = &mut canvas.layers[0];

    if w == src_w && h == src_h {
        // No scaling needed — copy directly into frame area
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                layer.buffer.set_pixel(fx0 + x, fy0 + y, Color::new(
                    src_rgba[i], src_rgba[i + 1], src_rgba[i + 2], src_rgba[i + 3],
                ));
            }
        }
    } else {
        // Nearest-neighbor downscale into frame area
        for y in 0..h {
            let src_y = (y as f64 / h as f64 * src_h as f64) as u32;
            for x in 0..w {
                let src_x = (x as f64 / w as f64 * src_w as f64) as u32;
                let i = ((src_y * src_w + src_x) * 4) as usize;
                layer.buffer.set_pixel(fx0 + x, fy0 + y, Color::new(
                    src_rgba[i], src_rgba[i + 1], src_rgba[i + 2], src_rgba[i + 3],
                ));
            }
        }
    }

    Ok(canvas)
}

/// Decode raw PNG frame data to RGBA regardless of source color type.
fn decode_to_rgba(img_data: &[u8], w: u32, h: u32, color_type: png::ColorType) -> Result<Vec<u8>, String> {
    let size = (w * h) as usize;
    let mut rgba = vec![0u8; size * 4];

    match color_type {
        png::ColorType::Rgba => {
            let needed = size * 4;
            if img_data.len() >= needed {
                rgba[..needed].copy_from_slice(&img_data[..needed]);
            } else {
                rgba[..img_data.len()].copy_from_slice(img_data);
            }
        }
        png::ColorType::Rgb => {
            for i in 0..size {
                let si = i * 3;
                let di = i * 4;
                if si + 2 < img_data.len() {
                    rgba[di] = img_data[si];
                    rgba[di + 1] = img_data[si + 1];
                    rgba[di + 2] = img_data[si + 2];
                    rgba[di + 3] = 255;
                }
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..size {
                if i < img_data.len() {
                    let v = img_data[i];
                    let di = i * 4;
                    rgba[di] = v;
                    rgba[di + 1] = v;
                    rgba[di + 2] = v;
                    rgba[di + 3] = 255;
                }
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..size {
                let si = i * 2;
                let di = i * 4;
                if si + 1 < img_data.len() {
                    let v = img_data[si];
                    rgba[di] = v;
                    rgba[di + 1] = v;
                    rgba[di + 2] = v;
                    rgba[di + 3] = img_data[si + 1];
                }
            }
        }
        _ => return Err(format!("Unsupported color type: {:?}", color_type)),
    }

    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_import_roundtrip() {
        let mut canvas = Canvas::new(4, 4);
        let red = Color::new(255, 0, 0, 255);
        // Use frame coordinates (add frame_x/y offset for buffer coords)
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0].buffer.set_pixel(fx + 1, fy + 1, red);
        canvas.layers[0].buffer.set_pixel(fx + 2, fy + 3, Color::new(0, 255, 0, 128));

        let png_data = export_png(&canvas).unwrap();
        assert!(!png_data.is_empty());

        let imported = import_png(&png_data).unwrap();
        assert_eq!(imported.frame_width(), 4);
        assert_eq!(imported.frame_height(), 4);
        // After import, pixel is at frame (1,1) = buffer (frame_x+1, frame_y+1)
        let ifx = imported.frame_x;
        let ify = imported.frame_y;
        assert_eq!(imported.layers[0].buffer.get_pixel(ifx + 1, ify + 1), Some(&red));
    }

    #[test]
    fn test_export_scaled() {
        let mut canvas = Canvas::new(2, 2);
        let red = Color::new(255, 0, 0, 255);
        // Set pixel at frame (0,0)
        canvas.layers[0].buffer.set_pixel(canvas.frame_x, canvas.frame_y, red);

        let png_data = export_png_scaled(&canvas, 4).unwrap();
        let imported = import_png(&png_data).unwrap();
        // 2x2 scaled by 4 = 8x8
        assert_eq!(imported.frame_width(), 8);
        assert_eq!(imported.frame_height(), 8);
        let ifx = imported.frame_x;
        let ify = imported.frame_y;
        assert_eq!(imported.layers[0].buffer.get_pixel(ifx, ify), Some(&red));
        assert_eq!(imported.layers[0].buffer.get_pixel(ifx + 3, ify + 3), Some(&red));
        assert_eq!(imported.layers[0].buffer.get_pixel(ifx + 4, ify), Some(&Color::TRANSPARENT));
    }

    #[test]
    fn test_import_downscale_large_image() {
        // Create a 512x512 canvas, export at 1x, then import with limit=128
        let mut canvas = Canvas::new(512, 512);
        let red = Color::new(255, 0, 0, 255);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        // Fill top-left quadrant of frame with red
        for y in 0..256 {
            for x in 0..256 {
                canvas.layers[0].buffer.set_pixel(fx + x, fy + y, red);
            }
        }
        let png_data = export_png(&canvas).unwrap();

        let imported = import_png_with_limit(&png_data, 128).unwrap();
        // 512x512 downscaled to 128x128
        assert_eq!(imported.frame_width(), 128);
        assert_eq!(imported.frame_height(), 128);
        let ifx = imported.frame_x;
        let ify = imported.frame_y;
        // Top-left should be red (sampled from red quadrant)
        assert_eq!(imported.layers[0].buffer.get_pixel(ifx, ify), Some(&red));
        // Bottom-right should be transparent (sampled from empty quadrant)
        assert_eq!(
            imported.layers[0].buffer.get_pixel(ifx + 127, ify + 127),
            Some(&Color::TRANSPARENT)
        );
    }

    #[test]
    fn test_import_within_limit_no_downscale() {
        let canvas = Canvas::new(64, 64);
        let png_data = export_png(&canvas).unwrap();
        let imported = import_png_with_limit(&png_data, 128).unwrap();
        // 64x64 < 128 limit, no downscale
        assert_eq!(imported.frame_width(), 64);
        assert_eq!(imported.frame_height(), 64);
    }

    #[test]
    fn test_import_non_square_downscale() {
        // 400x200 with limit=100 -> 100x50
        let canvas = Canvas::new(400, 200);
        let png_data = export_png(&canvas).unwrap();
        let imported = import_png_with_limit(&png_data, 100).unwrap();
        assert_eq!(imported.frame_width(), 100);
        assert_eq!(imported.frame_height(), 50);
    }
}
