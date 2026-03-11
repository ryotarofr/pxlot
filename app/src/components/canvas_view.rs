use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement, MouseEvent, WheelEvent};

use pixelforge_core::history::Command;
use pixelforge_core::Color;
use pixelforge_tools::{
    check_drawable, draw_ellipse, draw_filled_ellipse, draw_filled_rect, draw_line, draw_rect,
    ellipse_points, eyedropper, filled_ellipse_points, filled_rect_points, flood_fill,
    line_points, pencil_line, pencil_pixel, rect_points, ToolKind,
};

use crate::state::EditorState;

/// Draw onion skin overlays for previous/next frames.
fn draw_onion_skin(
    ctx: &CanvasRenderingContext2d,
    state: &EditorState,
    zoom: f64,
) {
    if !state.onion_skin || state.timeline.frames.len() <= 1 {
        return;
    }
    let cur = state.timeline.current_frame;
    let total = state.timeline.frames.len();
    let n = state.onion_skin_frames as usize;
    let w = state.canvas.width;
    let h = state.canvas.height;

    // Draw previous frames (blue tint)
    for offset in 1..=n {
        if offset > cur {
            break;
        }
        let idx = cur - offset;
        let opacity = 0.25 / offset as f64;
        let flat = state.timeline.frames[idx].canvas.flatten_visible();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) as usize) * 4;
                let a = flat[i + 3];
                if a > 0 {
                    let style = format!(
                        "rgba({},{},{},{})",
                        (flat[i] as f64 * 0.3) as u8,
                        (flat[i + 1] as f64 * 0.3) as u8,
                        255,
                        opacity * (a as f64 / 255.0)
                    );
                    ctx.set_fill_style_str(&style);
                    ctx.fill_rect(x as f64 * zoom, y as f64 * zoom, zoom, zoom);
                }
            }
        }
    }

    // Draw next frames (red tint)
    for offset in 1..=n {
        let idx = cur + offset;
        if idx >= total {
            break;
        }
        let opacity = 0.25 / offset as f64;
        let flat = state.timeline.frames[idx].canvas.flatten_visible();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) as usize) * 4;
                let a = flat[i + 3];
                if a > 0 {
                    let style = format!(
                        "rgba({},{},{},{})",
                        255,
                        (flat[i + 1] as f64 * 0.3) as u8,
                        (flat[i + 2] as f64 * 0.3) as u8,
                        opacity * (a as f64 / 255.0)
                    );
                    ctx.set_fill_style_str(&style);
                    ctx.fill_rect(x as f64 * zoom, y as f64 * zoom, zoom, zoom);
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

        // Apply pan offset via CSS transform
        editor.with_value(|state| {
            let html_el: &web_sys::HtmlElement = html_canvas.unchecked_ref();
            let css_style = html_el.style();
            let _ = css_style.set_property(
                "transform",
                &format!("translate({}px, {}px)", state.pan_x, state.pan_y),
            );
        });

        editor.with_value(|state| {
            let cw = state.canvas_display_width();
            let ch = state.canvas_display_height();

            html_canvas.set_width(cw as u32);
            html_canvas.set_height(ch as u32);

            let zoom = state.zoom;
            let fx0 = state.canvas.frame_x;
            let fy0 = state.canvas.frame_y;
            let fw = state.canvas.frame_width();
            let fh = state.canvas.frame_height();

            // Clear entire canvas with margin background
            ctx.set_fill_style_str("#1a1a2e");
            ctx.fill_rect(0.0, 0.0, cw, ch);

            // Draw checkerboard only inside frame area
            for y in 0..fh {
                for x in 0..fw {
                    let c1 = if (x + y) % 2 == 0 { "#2a2a3e" } else { "#222238" };
                    ctx.set_fill_style_str(c1);
                    ctx.fill_rect(
                        (fx0 + x) as f64 * zoom,
                        (fy0 + y) as f64 * zoom,
                        zoom,
                        zoom,
                    );
                }
            }

            // Draw onion skin (previous/next frames) behind current frame
            draw_onion_skin(&ctx, state, zoom);

            // Flatten and draw all visible layers (full buffer)
            let flat = state.canvas.flatten_visible();
            let w = state.canvas.width;
            let h = state.canvas.height;
            for y in 0..h {
                for x in 0..w {
                    let i = ((y * w + x) as usize) * 4;
                    let a = flat[i + 3];
                    if a > 0 {
                        let style = format!(
                            "rgba({},{},{},{})",
                            flat[i],
                            flat[i + 1],
                            flat[i + 2],
                            a as f64 / 255.0
                        );
                        ctx.set_fill_style_str(&style);
                        ctx.fill_rect(
                            x as f64 * zoom,
                            y as f64 * zoom,
                            zoom,
                            zoom,
                        );
                    }
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
                    let style = format!(
                        "rgba({},{},{},0.6)",
                        color.r, color.g, color.b
                    );
                    ctx.set_fill_style_str(&style);
                    for (px, py) in points {
                        // Allow preview in full buffer area, not just frame
                        if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                            ctx.fill_rect(
                                px as f64 * zoom,
                                py as f64 * zoom,
                                zoom,
                                zoom,
                            );
                        }
                    }
                }
            }

            // Helper to draw a dashed selection rectangle
            let draw_dashed_rect = |ctx: &CanvasRenderingContext2d, rx: f64, ry: f64, rw: f64, rh: f64| {
                ctx.set_stroke_style_str("rgba(74,158,255,0.8)");
                ctx.set_line_width(1.0);
                ctx.set_line_dash(&js_sys::Array::of2(
                    &wasm_bindgen::JsValue::from(4.0),
                    &wasm_bindgen::JsValue::from(4.0),
                )).ok();
                ctx.stroke_rect(rx, ry, rw, rh);
                ctx.set_line_dash(&js_sys::Array::new()).ok();
            };

            // Draw selection rectangle
            if let Some((sx, sy, sw, sh)) = state.selection {
                draw_dashed_rect(&ctx, sx as f64 * zoom, sy as f64 * zoom, sw as f64 * zoom, sh as f64 * zoom);
            }

            // Draw selection preview while dragging RectSelect
            if state.is_drawing && state.current_tool == ToolKind::RectSelect {
                let min_x = state.shape_start_x.min(state.last_draw_x);
                let min_y = state.shape_start_y.min(state.last_draw_y);
                let sel_w = (state.last_draw_x - state.shape_start_x).abs() + 1;
                let sel_h = (state.last_draw_y - state.shape_start_y).abs() + 1;
                draw_dashed_rect(&ctx, min_x as f64 * zoom, min_y as f64 * zoom, sel_w as f64 * zoom, sel_h as f64 * zoom);
            }

            // Draw frame border (shows the export area)
            ctx.set_stroke_style_str("rgba(0,100,200,0.7)");
            ctx.set_line_width(1.0);
            ctx.set_line_dash(&js_sys::Array::new()).ok();
            ctx.stroke_rect(
                fx0 as f64 * zoom - 0.5,
                fy0 as f64 * zoom - 0.5,
                fw as f64 * zoom + 1.0,
                fh as f64 * zoom + 1.0,
            );

            // Draw grid (only inside frame area)
            if state.show_grid && zoom >= 4.0 {
                let gs = state.grid_size.max(1) as u32;
                ctx.set_stroke_style_str("rgba(255,255,255,0.08)");
                ctx.set_line_width(0.5);
                let mut x = 0u32;
                while x <= fw {
                    ctx.begin_path();
                    ctx.move_to((fx0 + x) as f64 * zoom, fy0 as f64 * zoom);
                    ctx.line_to((fx0 + x) as f64 * zoom, (fy0 + fh) as f64 * zoom);
                    ctx.stroke();
                    x += gs;
                }
                let mut y = 0u32;
                while y <= fh {
                    ctx.begin_path();
                    ctx.move_to(fx0 as f64 * zoom, (fy0 + y) as f64 * zoom);
                    ctx.line_to((fx0 + fw) as f64 * zoom, (fy0 + y) as f64 * zoom);
                    ctx.stroke();
                    y += gs;
                }
                // Draw mirror axis if enabled (centered on frame)
                if state.mirror_x {
                    ctx.set_stroke_style_str("rgba(255,100,100,0.4)");
                    ctx.set_line_width(1.0);
                    let mid_x = (fx0 as f64 + fw as f64 / 2.0) * zoom;
                    ctx.begin_path();
                    ctx.move_to(mid_x, fy0 as f64 * zoom);
                    ctx.line_to(mid_x, (fy0 + fh) as f64 * zoom);
                    ctx.stroke();
                }
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
        let zoom = editor.with_value(|s| s.zoom);
        // Returns buffer coordinates (not frame coordinates)
        let px = ((ev.client_x() as f64 - rect.left()) / zoom) as i32;
        let py = ((ev.client_y() as f64 - rect.top()) / zoom) as i32;
        (px, py)
    };

    let on_mousedown = move |ev: MouseEvent| {
        ev.prevent_default();
        let (px, py) = mouse_to_pixel(&ev);
        let right_click = ev.button() == 2;

        editor.update_value(|state| {
            if ev.button() == 1 {
                state.is_panning = true;
                state.pan_last_mouse_x = ev.client_x() as f64;
                state.pan_last_mouse_y = ev.client_y() as f64;
                return;
            }

            // Right-click always erases (transparent), regardless of tool
            let effective_tool = if right_click { ToolKind::Eraser } else { state.current_tool };

            // Check if drawing tools can operate on the active layer
            let needs_draw = matches!(
                effective_tool,
                ToolKind::Pencil | ToolKind::Eraser | ToolKind::Fill
                    | ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse
                    | ToolKind::FilledRectangle | ToolKind::FilledEllipse
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
                            let frame_center_x = state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
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
                            let frame_center_x = state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
                            let mx = frame_center_x - 1 - px;
                            if mx >= 0 {
                                pencil_pixel(&mut state.canvas, mx as u32, py as u32, color, &mut cmd);
                            }
                        }
                    }
                    drag_cmd.set_value(Some(cmd));
                }
                ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse
                | ToolKind::FilledRectangle | ToolKind::FilledEllipse => {
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
    };

    let on_mousemove = move |ev: MouseEvent| {
        // Handle panning with middle mouse button
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

        let (px, py) = mouse_to_pixel(&ev);

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
                    let frame_center_x = state.canvas.frame_x as i32 * 2 + state.canvas.frame_width() as i32;
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
                ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse
                | ToolKind::FilledRectangle | ToolKind::FilledEllipse | ToolKind::RectSelect => {
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
                        // Clamp selection to buffer bounds
                        let min_x = state.shape_start_x.min(state.last_draw_x).max(0);
                        let min_y = state.shape_start_y.min(state.last_draw_y).max(0);
                        let max_x = state.shape_start_x.max(state.last_draw_x)
                            .min(state.canvas.width as i32 - 1);
                        let max_y = state.shape_start_y.max(state.last_draw_y)
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

            // Mouse position relative to the canvas element's bounding rect
            let mouse_x = ev.client_x() as f64 - rect.left();
            let mouse_y = ev.client_y() as f64 - rect.top();

            // Adjust pan so the pixel under the cursor stays in place.
            let cw = state.canvas.width as f64;
            let ch = state.canvas.height as f64;
            state.pan_x += mouse_x * (1.0 - new_zoom / old_zoom)
                + cw * (new_zoom - old_zoom) / 2.0;
            state.pan_y += mouse_y * (1.0 - new_zoom / old_zoom)
                + ch * (new_zoom - old_zoom) / 2.0;
            state.zoom = new_zoom;
        });
        render();
    };

    // Prevent default context menu on middle-click / right-click
    let on_auxclick = move |ev: MouseEvent| {
        if ev.button() == 1 {
            ev.prevent_default();
        }
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
            on:mouseleave=on_mouseup
            on:wheel=on_wheel
            on:auxclick=on_auxclick
            on:contextmenu=on_contextmenu
        />
    }
}
