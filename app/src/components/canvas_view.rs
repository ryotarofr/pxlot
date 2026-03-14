use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement, MouseEvent, WheelEvent};

use pxlot_core::Color;
use pxlot_core::history::Command;
use pxlot_tools::{
    ToolKind, check_drawable, draw_ellipse, draw_filled_ellipse, draw_filled_rect, draw_line,
    draw_rect, ellipse_points, eyedropper, filled_ellipse_points, filled_rect_points, flood_fill,
    line_points, pencil_line, pencil_pixel, rect_points,
};

use crate::state::EditorState;
use crate::storage;

/// Alpha-blend `src` over `dst` (both premultiplied-style RGBA), returns final RGBA bytes.
#[inline]
fn blend_over(dst: [u8; 4], src_r: u8, src_g: u8, src_b: u8, src_a: u8) -> [u8; 4] {
    if src_a == 0 {
        return dst;
    }
    if src_a == 255 {
        return [src_r, src_g, src_b, 255];
    }
    let sa = src_a as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a < 0.001 {
        return [0, 0, 0, 0];
    }
    let r = (src_r as f32 * sa + dst[0] as f32 * da * (1.0 - sa)) / out_a;
    let g = (src_g as f32 * sa + dst[1] as f32 * da * (1.0 - sa)) / out_a;
    let b = (src_b as f32 * sa + dst[2] as f32 * da * (1.0 - sa)) / out_a;
    [r as u8, g as u8, b as u8, (out_a * 255.0) as u8]
}

/// Draw onion skin into an image buffer (at 1:1 pixel scale).
fn draw_onion_skin_to_buffer(
    buf: &mut [u8],
    buf_stride: usize,
    state: &EditorState,
    vx0: i32,
    vy0: i32,
    vx1: i32,
    vy1: i32,
) {
    if !state.onion_skin || state.timeline.frames.len() <= 1 {
        return;
    }
    let cur = state.timeline.current_frame;
    let total = state.timeline.frames.len();
    let n = state.onion_skin_frames as usize;
    let rw = (vx1 - vx0) as usize;

    // Previous frames (blue tint)
    for offset in 1..=n {
        if offset > cur {
            break;
        }
        let idx = cur - offset;
        let opacity = 0.25 / offset as f64;
        let flat = state.timeline.frames[idx]
            .canvas
            .flatten_region(vx0 as u32, vy0 as u32, vx1 as u32, vy1 as u32);
        for ly in 0..((vy1 - vy0) as usize) {
            for lx in 0..rw {
                let i = (ly * rw + lx) * 4;
                let a = flat[i + 3];
                if a > 0 {
                    let sa = (opacity * (a as f64 / 255.0) * 255.0) as u8;
                    let sr = (flat[i] as f64 * 0.3) as u8;
                    let sg = (flat[i + 1] as f64 * 0.3) as u8;
                    let sb = 255u8;
                    let bi = (ly * buf_stride + lx) * 4;
                    let dst = [buf[bi], buf[bi + 1], buf[bi + 2], buf[bi + 3]];
                    let out = blend_over(dst, sr, sg, sb, sa);
                    buf[bi..bi + 4].copy_from_slice(&out);
                }
            }
        }
    }

    // Next frames (red tint)
    for offset in 1..=n {
        let idx = cur + offset;
        if idx >= total {
            break;
        }
        let opacity = 0.25 / offset as f64;
        let flat = state.timeline.frames[idx]
            .canvas
            .flatten_region(vx0 as u32, vy0 as u32, vx1 as u32, vy1 as u32);
        for ly in 0..((vy1 - vy0) as usize) {
            for lx in 0..rw {
                let i = (ly * rw + lx) * 4;
                let a = flat[i + 3];
                if a > 0 {
                    let sa = (opacity * (a as f64 / 255.0) * 255.0) as u8;
                    let sr = 255u8;
                    let sg = (flat[i + 1] as f64 * 0.3) as u8;
                    let sb = (flat[i + 2] as f64 * 0.3) as u8;
                    let bi = (ly * buf_stride + lx) * 4;
                    let dst = [buf[bi], buf[bi + 1], buf[bi + 2], buf[bi + 3]];
                    let out = blend_over(dst, sr, sg, sb, sa);
                    buf[bi..bi + 4].copy_from_slice(&out);
                }
            }
        }
    }
}

#[component]
pub fn CanvasView(
    editor: StoredValue<EditorState>,
    render_trigger: ReadSignal<u32>,
    set_color: WriteSignal<Color>,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let drag_cmd = StoredValue::new(Option::<Command>::None);

    // Render function
    let render = move || {
        let Some(el) = canvas_ref.get() else {
            return;
        };
        let html_canvas: &HtmlCanvasElement = el.as_ref();

        let ctx: Option<CanvasRenderingContext2d> = html_canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into::<CanvasRenderingContext2d>().ok());

        let Some(ctx) = ctx else { return };

        // Get viewport size from parent element
        let element: &Element = html_canvas.unchecked_ref();
        let parent = element.parent_element();
        let (vp_w, vp_h) = if let Some(ref p) = parent {
            (p.client_width() as f64, p.client_height() as f64)
        } else {
            (800.0, 600.0)
        };

        // Set canvas element to fill viewport
        html_canvas.set_width(vp_w as u32);
        html_canvas.set_height(vp_h as u32);

        // Center on first render
        editor.update_value(|state| {
            if state.needs_center {
                state.center_on_frame(vp_w, vp_h);
                state.needs_center = false;
            }
        });

        // Update cursor style based on state
        let cursor = editor.with_value(|state| {
            if state.is_panning || state.space_held {
                if state.is_panning { "grabbing" } else { "grab" }.to_string()
            } else {
                match state.current_tool {
                    ToolKind::Eyedropper => "crosshair".to_string(),
                    ToolKind::Fill => "cell".to_string(),
                    _ => "crosshair".to_string(),
                }
            }
        });
        let html_el: &web_sys::HtmlElement = html_canvas.unchecked_ref();
        let _ = html_el.style().set_property("cursor", &cursor);

        editor.with_value(|state| {
            let zoom = state.zoom;
            let pan_x = state.pan_x;
            let pan_y = state.pan_y;
            let fx0 = state.canvas.frame_x;
            let fy0 = state.canvas.frame_y;
            let fw = state.canvas.frame_width();
            let fh = state.canvas.frame_height();
            let buf_w = state.canvas.width;
            let buf_h = state.canvas.height;

            // Clear entire viewport with uniform background
            ctx.set_fill_style_str("#161628");
            ctx.fill_rect(0.0, 0.0, vp_w, vp_h);

            // Calculate visible buffer pixel range (culling)
            let vx0 = ((-pan_x) / zoom).floor() as i32;
            let vy0 = ((-pan_y) / zoom).floor() as i32;
            let vx1 = ((-pan_x + vp_w) / zoom).ceil() as i32;
            let vy1 = ((-pan_y + vp_h) / zoom).ceil() as i32;

            // Clamp to buffer bounds
            let vx0 = vx0.max(0).min(buf_w as i32);
            let vy0 = vy0.max(0).min(buf_h as i32);
            let vx1 = vx1.max(0).min(buf_w as i32);
            let vy1 = vy1.max(0).min(buf_h as i32);

            let vis_w = (vx1 - vx0) as usize;
            let vis_h = (vy1 - vy0) as usize;

            if vis_w > 0 && vis_h > 0 {
                // Build composited pixel buffer: checkerboard + pixel data
                let mut img_data = vec![255u8; vis_w * vis_h * 4];

                // Checkerboard colors
                let cb_a: [u8; 3] = [0x1e, 0x1e, 0x38];
                let cb_b: [u8; 3] = [0x1a, 0x1a, 0x32];

                // Fill with checkerboard
                for ly in 0..vis_h {
                    let wy = vy0 + ly as i32;
                    for lx in 0..vis_w {
                        let wx = vx0 + lx as i32;
                        let i = (ly * vis_w + lx) * 4;
                        let cb = if ((wx + wy) % 2) == 0 { &cb_a } else { &cb_b };
                        img_data[i] = cb[0];
                        img_data[i + 1] = cb[1];
                        img_data[i + 2] = cb[2];
                        img_data[i + 3] = 255;
                    }
                }

                // Draw onion skin into buffer
                draw_onion_skin_to_buffer(&mut img_data, vis_w, state, vx0, vy0, vx1, vy1);

                // Blend pixel data on top (only visible region)
                let flat = state
                    .canvas
                    .flatten_region(vx0 as u32, vy0 as u32, vx1 as u32, vy1 as u32);
                for ly in 0..vis_h {
                    for lx in 0..vis_w {
                        let fi = (ly * vis_w + lx) * 4;
                        let a = flat[fi + 3];
                        if a > 0 {
                            let i = fi;
                            let dst = [
                                img_data[i],
                                img_data[i + 1],
                                img_data[i + 2],
                                img_data[i + 3],
                            ];
                            let out = blend_over(dst, flat[fi], flat[fi + 1], flat[fi + 2], a);
                            img_data[i..i + 4].copy_from_slice(&out);
                        }
                    }
                }

                // Create offscreen canvas and draw ImageData, then scale to viewport
                let document = web_sys::window().unwrap().document().unwrap();
                let offscreen: HtmlCanvasElement =
                    document.create_element("canvas").unwrap().unchecked_into();
                offscreen.set_width(vis_w as u32);
                offscreen.set_height(vis_h as u32);
                let off_ctx: CanvasRenderingContext2d = offscreen
                    .get_context("2d")
                    .ok()
                    .flatten()
                    .unwrap()
                    .unchecked_into();

                // Create ImageData from buffer
                let clamped = wasm_bindgen::Clamped(&img_data[..]);
                if let Ok(image_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
                    clamped,
                    vis_w as u32,
                    vis_h as u32,
                ) {
                    let _ = off_ctx.put_image_data(&image_data, 0.0, 0.0);

                    // Draw scaled to main canvas
                    ctx.set_image_smoothing_enabled(false);
                    let dx = vx0 as f64 * zoom + pan_x;
                    let dy = vy0 as f64 * zoom + pan_y;
                    let dw = vis_w as f64 * zoom;
                    let dh = vis_h as f64 * zoom;
                    let _ = ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
                        &offscreen, dx, dy, dw, dh,
                    );
                }
            }

            // Draw shape preview if dragging a shape tool
            if state.is_drawing {
                let preview_points = match state.current_tool {
                    ToolKind::Line => Some(line_points(
                        state.shape_start_x,
                        state.shape_start_y,
                        state.last_draw_x,
                        state.last_draw_y,
                    )),
                    ToolKind::Rectangle => Some(rect_points(
                        state.shape_start_x,
                        state.shape_start_y,
                        state.last_draw_x,
                        state.last_draw_y,
                    )),
                    ToolKind::Ellipse => Some(ellipse_points(
                        state.shape_start_x,
                        state.shape_start_y,
                        state.last_draw_x,
                        state.last_draw_y,
                    )),
                    ToolKind::FilledRectangle => Some(filled_rect_points(
                        state.shape_start_x,
                        state.shape_start_y,
                        state.last_draw_x,
                        state.last_draw_y,
                    )),
                    ToolKind::FilledEllipse => Some(filled_ellipse_points(
                        state.shape_start_x,
                        state.shape_start_y,
                        state.last_draw_x,
                        state.last_draw_y,
                    )),
                    _ => None,
                };

                if let Some(points) = preview_points {
                    let color = state.current_color;
                    let style = format!("rgba({},{},{},0.6)", color.r, color.g, color.b);
                    ctx.set_fill_style_str(&style);
                    for (px, py) in points {
                        if px >= 0 && py >= 0 && (px as u32) < buf_w && (py as u32) < buf_h {
                            ctx.fill_rect(
                                px as f64 * zoom + pan_x,
                                py as f64 * zoom + pan_y,
                                zoom,
                                zoom,
                            );
                        }
                    }
                }
            }

            // Helper to draw a dashed selection rectangle
            let draw_dashed_rect =
                |ctx: &CanvasRenderingContext2d, rx: f64, ry: f64, rw: f64, rh: f64| {
                    ctx.set_stroke_style_str("rgba(74,158,255,0.8)");
                    ctx.set_line_width(1.0);
                    ctx.set_line_dash(&js_sys::Array::of2(
                        &wasm_bindgen::JsValue::from(4.0),
                        &wasm_bindgen::JsValue::from(4.0),
                    ))
                    .ok();
                    ctx.stroke_rect(rx, ry, rw, rh);
                    ctx.set_line_dash(&js_sys::Array::new()).ok();
                };

            // Draw selection rectangle
            if let Some((sx, sy, sw, sh)) = state.selection {
                draw_dashed_rect(
                    &ctx,
                    sx as f64 * zoom + pan_x,
                    sy as f64 * zoom + pan_y,
                    sw as f64 * zoom,
                    sh as f64 * zoom,
                );
            }

            // Draw selection preview while dragging RectSelect
            if state.is_drawing && state.current_tool == ToolKind::RectSelect {
                let min_x = state.shape_start_x.min(state.last_draw_x);
                let min_y = state.shape_start_y.min(state.last_draw_y);
                let sel_w = (state.last_draw_x - state.shape_start_x).abs() + 1;
                let sel_h = (state.last_draw_y - state.shape_start_y).abs() + 1;
                draw_dashed_rect(
                    &ctx,
                    min_x as f64 * zoom + pan_x,
                    min_y as f64 * zoom + pan_y,
                    sel_w as f64 * zoom,
                    sel_h as f64 * zoom,
                );
            }

            // Draw grid across entire visible area (batched paths)
            if state.show_grid && zoom >= 4.0 {
                let gs = state.grid_size.max(1) as u32;

                let gx0 = vx0 as u32;
                let gy0 = vy0 as u32;
                let gx1 = vx1 as u32;
                let gy1 = vy1 as u32;
                let gy0f = gy0 as f64 * zoom + pan_y;
                let gy1f = gy1 as f64 * zoom + pan_y;
                let gx0f = gx0 as f64 * zoom + pan_x;
                let gx1f = gx1 as f64 * zoom + pan_x;

                // Fine grid lines - single batched path
                ctx.set_stroke_style_str("rgba(255,255,255,0.05)");
                ctx.set_line_width(0.5);
                ctx.begin_path();

                let start_x = (gx0 / gs) * gs;
                let mut x = start_x;
                while x <= gx1 {
                    let xf = x as f64 * zoom + pan_x;
                    ctx.move_to(xf, gy0f);
                    ctx.line_to(xf, gy1f);
                    x += gs;
                }
                let start_y = (gy0 / gs) * gs;
                let mut y = start_y;
                while y <= gy1 {
                    let yf = y as f64 * zoom + pan_y;
                    ctx.move_to(gx0f, yf);
                    ctx.line_to(gx1f, yf);
                    y += gs;
                }
                ctx.stroke();

                // Coarse grid lines (every 8 pixels) - single batched path
                let coarse = (gs * 8).max(8);
                ctx.set_stroke_style_str("rgba(255,255,255,0.10)");
                ctx.set_line_width(0.5);
                ctx.begin_path();

                let start_cx = (gx0 / coarse) * coarse;
                let mut cx = start_cx;
                while cx <= gx1 {
                    let xf = cx as f64 * zoom + pan_x;
                    ctx.move_to(xf, gy0f);
                    ctx.line_to(xf, gy1f);
                    cx += coarse;
                }
                let start_cy = (gy0 / coarse) * coarse;
                let mut cy = start_cy;
                while cy <= gy1 {
                    let yf = cy as f64 * zoom + pan_y;
                    ctx.move_to(gx0f, yf);
                    ctx.line_to(gx1f, yf);
                    cy += coarse;
                }
                ctx.stroke();
            }

            // Frame boundary: smooth thick border lines
            let frame_left = fx0 as f64 * zoom + pan_x;
            let frame_top = fy0 as f64 * zoom + pan_y;
            let frame_right = (fx0 + fw) as f64 * zoom + pan_x;
            let frame_bottom = (fy0 + fh) as f64 * zoom + pan_y;

            ctx.set_stroke_style_str("rgba(100,160,220,0.5)");
            ctx.set_line_width(2.0);
            ctx.set_line_dash(&js_sys::Array::new()).ok();
            ctx.stroke_rect(
                frame_left,
                frame_top,
                frame_right - frame_left,
                frame_bottom - frame_top,
            );

            // Corner markers (small filled squares at each corner)
            let corner_size = (zoom * 0.6).clamp(3.0, 12.0);
            ctx.set_fill_style_str("rgba(80,140,200,0.7)");
            ctx.fill_rect(
                frame_left - corner_size * 0.5,
                frame_top - corner_size * 0.5,
                corner_size,
                corner_size,
            );
            ctx.fill_rect(
                frame_right - corner_size * 0.5,
                frame_top - corner_size * 0.5,
                corner_size,
                corner_size,
            );
            ctx.fill_rect(
                frame_left - corner_size * 0.5,
                frame_bottom - corner_size * 0.5,
                corner_size,
                corner_size,
            );
            ctx.fill_rect(
                frame_right - corner_size * 0.5,
                frame_bottom - corner_size * 0.5,
                corner_size,
                corner_size,
            );

            // Dim area outside the frame (semi-transparent overlay)
            {
                ctx.set_fill_style_str("rgba(0,0,10,0.35)");
                if frame_top > 0.0 {
                    ctx.fill_rect(0.0, 0.0, vp_w, frame_top);
                }
                if frame_bottom < vp_h {
                    ctx.fill_rect(0.0, frame_bottom, vp_w, vp_h - frame_bottom);
                }
                let strip_top = frame_top.max(0.0);
                let strip_bottom = frame_bottom.min(vp_h);
                if frame_left > 0.0 && strip_bottom > strip_top {
                    ctx.fill_rect(0.0, strip_top, frame_left, strip_bottom - strip_top);
                }
                if frame_right < vp_w && strip_bottom > strip_top {
                    ctx.fill_rect(
                        frame_right,
                        strip_top,
                        vp_w - frame_right,
                        strip_bottom - strip_top,
                    );
                }
            }

            // Pixel cursor highlight
            if state.hover_x >= 0
                && state.hover_y >= 0
                && (state.hover_x as u32) < buf_w
                && (state.hover_y as u32) < buf_h
            {
                let hx = state.hover_x as f64 * zoom + pan_x;
                let hy = state.hover_y as f64 * zoom + pan_y;
                ctx.set_stroke_style_str("rgba(255,255,255,0.6)");
                ctx.set_line_width(1.0);
                ctx.stroke_rect(hx + 0.5, hy + 0.5, zoom - 1.0, zoom - 1.0);
            }

            // Coordinate label near cursor (relative to frame origin)
            if state.hover_x >= 0
                && state.hover_y >= 0
                && (state.hover_x as u32) < buf_w
                && (state.hover_y as u32) < buf_h
            {
                let rel_x = state.hover_x - fx0 as i32;
                let rel_y = state.hover_y - fy0 as i32;
                let label = format!("{},{}", rel_x, rel_y);
                let lx = state.hover_x as f64 * zoom + pan_x + zoom + 6.0;
                let ly = state.hover_y as f64 * zoom + pan_y - 4.0;
                ctx.set_font("11px monospace");
                let tw = label.len() as f64 * 6.6;
                ctx.set_fill_style_str("rgba(0,0,0,0.7)");
                ctx.fill_rect(lx - 2.0, ly - 10.0, tw + 4.0, 14.0);
                let in_frame =
                    rel_x >= 0 && rel_y >= 0 && (rel_x as u32) < fw && (rel_y as u32) < fh;
                if in_frame {
                    ctx.set_fill_style_str("rgba(200,220,255,0.9)");
                } else {
                    ctx.set_fill_style_str("rgba(255,180,100,0.9)");
                }
                ctx.fill_text(&label, lx, ly).ok();
            }

            // Draw mirror axis if enabled (centered on frame)
            if state.mirror_x {
                ctx.set_stroke_style_str("rgba(255,100,100,0.4)");
                ctx.set_line_width(1.0);
                let mid_x = (fx0 as f64 + fw as f64 / 2.0) * zoom + pan_x;
                ctx.begin_path();
                ctx.move_to(mid_x, frame_top);
                ctx.line_to(mid_x, frame_bottom);
                ctx.stroke();
            }
        });
    };

    // Re-render when trigger changes
    Effect::new(move |_| {
        let _ = render_trigger.get();
        render();
    });

    // Convert mouse position to buffer pixel coordinates
    let mouse_to_pixel = move |ev: &MouseEvent| -> (i32, i32) {
        let Some(el) = canvas_ref.get() else {
            return (-1, -1);
        };
        let html_canvas: &HtmlCanvasElement = el.as_ref();
        let element: &Element = html_canvas.unchecked_ref();
        let rect = element.get_bounding_client_rect();
        let (zoom, pan_x, pan_y) = editor.with_value(|s| (s.zoom, s.pan_x, s.pan_y));
        let sx = ev.client_x() as f64 - rect.left();
        let sy = ev.client_y() as f64 - rect.top();
        let px = ((sx - pan_x) / zoom).floor() as i32;
        let py = ((sy - pan_y) / zoom).floor() as i32;
        (px, py)
    };

    let on_mousedown = move |ev: MouseEvent| {
        ev.prevent_default();
        // Ensure the parent .app div keeps focus for keyboard shortcuts
        let target_js = ev.target().unwrap();
        let target: &Element = target_js.unchecked_ref();
        if let Some(app_div) = target.closest(".app").ok().flatten() {
            let html_el: &web_sys::HtmlElement = app_div.unchecked_ref();
            let _ = html_el.focus();
        }
        let (px, py) = mouse_to_pixel(&ev);
        let right_click = ev.button() == 2;

        editor.update_value(|state| {
            // Middle mouse button or space+left-click = panning
            if ev.button() == 1 || (ev.button() == 0 && state.space_held) {
                state.is_panning = true;
                state.pan_last_mouse_x = ev.client_x() as f64;
                state.pan_last_mouse_y = ev.client_y() as f64;
                return;
            }

            let effective_tool = if right_click {
                ToolKind::Eraser
            } else {
                state.current_tool
            };

            let needs_draw = matches!(
                effective_tool,
                ToolKind::Pencil
                    | ToolKind::Eraser
                    | ToolKind::Fill
                    | ToolKind::Line
                    | ToolKind::Rectangle
                    | ToolKind::Ellipse
                    | ToolKind::FilledRectangle
                    | ToolKind::FilledEllipse
            );
            if needs_draw {
                if let Some(reason) = check_drawable(&state.canvas) {
                    state.status_message = Some(reason.to_string());
                    return;
                }
            }

            match effective_tool {
                ToolKind::Eyedropper => {
                    if px >= 0 && py >= 0 {
                        if let Some(c) = eyedropper(&state.canvas, px as u32, py as u32) {
                            set_color.set(c);
                        }
                    }
                }
                ToolKind::Fill => {
                    if px >= 0 && py >= 0 {
                        let mut cmd = Command::new("Fill");
                        flood_fill(
                            &mut state.canvas,
                            px as u32,
                            py as u32,
                            state.current_color,
                            &mut cmd,
                        );
                        if state.mirror_x {
                            let frame_center_x =
                                state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
                            let mx = frame_center_x - 1 - px;
                            if mx >= 0 {
                                flood_fill(
                                    &mut state.canvas,
                                    mx as u32,
                                    py as u32,
                                    state.current_color,
                                    &mut cmd,
                                );
                            }
                        }
                        state.history.push(cmd);
                    }
                }
                ToolKind::Pencil | ToolKind::Eraser => {
                    state.is_drawing = true;
                    state.last_draw_x = px;
                    state.last_draw_y = py;
                    let color = if effective_tool == ToolKind::Eraser {
                        Color::TRANSPARENT
                    } else {
                        state.current_color
                    };
                    let tool_name = if effective_tool == ToolKind::Eraser {
                        "Erase"
                    } else {
                        "Draw"
                    };
                    let mut cmd = Command::new(tool_name);
                    if px >= 0 && py >= 0 {
                        pencil_pixel(&mut state.canvas, px as u32, py as u32, color, &mut cmd);
                        if state.mirror_x {
                            let frame_center_x =
                                state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
                            let mx = frame_center_x - 1 - px;
                            if mx >= 0 {
                                pencil_pixel(
                                    &mut state.canvas,
                                    mx as u32,
                                    py as u32,
                                    color,
                                    &mut cmd,
                                );
                            }
                        }
                    }
                    drag_cmd.set_value(Some(cmd));
                }
                ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::FilledRectangle
                | ToolKind::FilledEllipse => {
                    state.is_drawing = true;
                    state.shape_start_x = px;
                    state.shape_start_y = py;
                    state.last_draw_x = px;
                    state.last_draw_y = py;
                }
                ToolKind::RectSelect => {
                    state.is_drawing = true;
                    state.shape_start_x = px;
                    state.shape_start_y = py;
                    state.last_draw_x = px;
                    state.last_draw_y = py;
                    state.selection = None;
                }
            }
        });
        render();
        // Autosave after immediate operations (fill, etc.)
        editor.with_value(|state| {
            storage::autosave(&state.canvas, &state.history);
        });
    };

    let on_mousemove = move |ev: MouseEvent| {
        let (hx, hy) = mouse_to_pixel(&ev);
        editor.update_value(|state| {
            state.hover_x = hx;
            state.hover_y = hy;
        });

        let is_panning = editor.with_value(|s| s.is_panning);
        if is_panning {
            editor.update_value(|state| {
                let dx = ev.client_x() as f64 - state.pan_last_mouse_x;
                let dy = ev.client_y() as f64 - state.pan_last_mouse_y;
                state.pan_x += dx;
                state.pan_y += dy;
                state.pan_last_mouse_x = ev.client_x() as f64;
                state.pan_last_mouse_y = ev.client_y() as f64;
            });
            render();
            return;
        }

        let (px, py) = (hx, hy);

        editor.update_value(|state| {
            if !state.is_drawing {
                return;
            }
            match state.current_tool {
                ToolKind::Pencil | ToolKind::Eraser => {
                    let color = if state.current_tool == ToolKind::Eraser {
                        Color::TRANSPARENT
                    } else {
                        state.current_color
                    };
                    let mirror = state.mirror_x;
                    let frame_center_x =
                        state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
                    drag_cmd.update_value(|opt_cmd| {
                        if let Some(cmd) = opt_cmd {
                            pencil_line(
                                &mut state.canvas,
                                state.last_draw_x,
                                state.last_draw_y,
                                px,
                                py,
                                color,
                                cmd,
                            );
                            if mirror {
                                let mx0 = frame_center_x - 1 - state.last_draw_x;
                                let mx1 = frame_center_x - 1 - px;
                                pencil_line(
                                    &mut state.canvas,
                                    mx0,
                                    state.last_draw_y,
                                    mx1,
                                    py,
                                    color,
                                    cmd,
                                );
                            }
                        }
                    });
                    state.last_draw_x = px;
                    state.last_draw_y = py;
                }
                ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::FilledRectangle
                | ToolKind::FilledEllipse
                | ToolKind::RectSelect => {
                    state.last_draw_x = px;
                    state.last_draw_y = py;
                }
                _ => {}
            }
        });
        render();
    };

    let on_mouseup = move |_ev: MouseEvent| {
        editor.update_value(|state| {
            if state.is_drawing {
                state.is_drawing = false;

                match state.current_tool {
                    ToolKind::Pencil | ToolKind::Eraser => {
                        drag_cmd.update_value(|opt_cmd| {
                            if let Some(cmd) = opt_cmd.take() {
                                state.history.push(cmd);
                            }
                        });
                    }
                    ToolKind::Line => {
                        let mut cmd = Command::new("Line");
                        draw_line(
                            &mut state.canvas,
                            state.shape_start_x,
                            state.shape_start_y,
                            state.last_draw_x,
                            state.last_draw_y,
                            state.current_color,
                            &mut cmd,
                        );
                        state.history.push(cmd);
                    }
                    ToolKind::Rectangle => {
                        let mut cmd = Command::new("Rectangle");
                        draw_rect(
                            &mut state.canvas,
                            state.shape_start_x,
                            state.shape_start_y,
                            state.last_draw_x,
                            state.last_draw_y,
                            state.current_color,
                            &mut cmd,
                        );
                        state.history.push(cmd);
                    }
                    ToolKind::Ellipse => {
                        let mut cmd = Command::new("Ellipse");
                        draw_ellipse(
                            &mut state.canvas,
                            state.shape_start_x,
                            state.shape_start_y,
                            state.last_draw_x,
                            state.last_draw_y,
                            state.current_color,
                            &mut cmd,
                        );
                        state.history.push(cmd);
                    }
                    ToolKind::FilledRectangle => {
                        let mut cmd = Command::new("FilledRect");
                        draw_filled_rect(
                            &mut state.canvas,
                            state.shape_start_x,
                            state.shape_start_y,
                            state.last_draw_x,
                            state.last_draw_y,
                            state.current_color,
                            &mut cmd,
                        );
                        state.history.push(cmd);
                    }
                    ToolKind::FilledEllipse => {
                        let mut cmd = Command::new("FilledEllipse");
                        draw_filled_ellipse(
                            &mut state.canvas,
                            state.shape_start_x,
                            state.shape_start_y,
                            state.last_draw_x,
                            state.last_draw_y,
                            state.current_color,
                            &mut cmd,
                        );
                        state.history.push(cmd);
                    }
                    ToolKind::RectSelect => {
                        let min_x = state.shape_start_x.min(state.last_draw_x).max(0);
                        let min_y = state.shape_start_y.min(state.last_draw_y).max(0);
                        let max_x = state
                            .shape_start_x
                            .max(state.last_draw_x)
                            .min(state.canvas.width as i32 - 1);
                        let max_y = state
                            .shape_start_y
                            .max(state.last_draw_y)
                            .min(state.canvas.height as i32 - 1);
                        let sel_w = max_x - min_x + 1;
                        let sel_h = max_y - min_y + 1;
                        if sel_w > 0 && sel_h > 0 {
                            state.selection = Some((min_x, min_y, sel_w, sel_h));
                        }
                    }
                    _ => {}
                }
            }
            state.is_panning = false;
        });
        render();
        // Autosave after drawing completes
        editor.with_value(|state| {
            storage::autosave(&state.canvas, &state.history);
        });
    };

    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let Some(el) = canvas_ref.get() else { return };
        let html_canvas: &HtmlCanvasElement = el.as_ref();
        let element: &Element = html_canvas.unchecked_ref();
        let rect = element.get_bounding_client_rect();

        editor.update_value(|state| {
            let old_zoom = state.zoom;
            let delta = if ev.delta_y() < 0.0 { 1.2 } else { 1.0 / 1.2 };
            let new_zoom = (old_zoom * delta).clamp(1.0, 64.0);

            let mouse_x = ev.client_x() as f64 - rect.left();
            let mouse_y = ev.client_y() as f64 - rect.top();

            let world_x = (mouse_x - state.pan_x) / old_zoom;
            let world_y = (mouse_y - state.pan_y) / old_zoom;

            state.pan_x = mouse_x - world_x * new_zoom;
            state.pan_y = mouse_y - world_y * new_zoom;
            state.zoom = new_zoom;
        });
        render();
    };

    let on_auxclick = move |ev: MouseEvent| {
        if ev.button() == 1 {
            ev.prevent_default();
        }
    };

    let on_mouseleave_hover = move |ev: MouseEvent| {
        on_mouseup(ev);
        editor.update_value(|state| {
            state.hover_x = -1;
            state.hover_y = -1;
        });
        render();
    };

    let on_contextmenu = move |ev: MouseEvent| {
        ev.prevent_default();
    };

    view! {
        <canvas
            node_ref=canvas_ref
            class="pixel-canvas"
            on:mousedown=on_mousedown
            on:mousemove=on_mousemove
            on:mouseup=on_mouseup
            on:mouseleave=on_mouseleave_hover
            on:wheel=on_wheel
            on:auxclick=on_auxclick
            on:contextmenu=on_contextmenu
        />
    }
}
