/// Tool definitions and execution bridge for the AI agent.
/// Maps LLM tool_use calls to pxlot_tools / Canvas operations.
use pxlot_core::history::Command;
use pxlot_core::{Canvas, Color};
use serde_json::{Value, json};

use super::api_client::ToolDefinition;

// ── Tool definitions sent to the API ───────────────────────────

#[allow(dead_code)]
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "set_pixels".into(),
            description: "Set one or more pixels. Efficient for batch operations.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pixels": {
                        "type": "array",
                        "description": "Array of pixels to set",
                        "items": {
                            "type": "object",
                            "properties": {
                                "x": { "type": "integer", "description": "X coordinate (0 = left)" },
                                "y": { "type": "integer", "description": "Y coordinate (0 = top)" },
                                "color": { "type": "string", "description": "Hex color e.g. #ff0000" }
                            },
                            "required": ["x", "y", "color"]
                        }
                    }
                },
                "required": ["pixels"]
            }),
        },
        ToolDefinition {
            name: "draw_line".into(),
            description: "Draw a straight line between two points.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string", "description": "Hex color" }
                },
                "required": ["x0", "y0", "x1", "y1", "color"]
            }),
        },
        ToolDefinition {
            name: "draw_rect".into(),
            description: "Draw a rectangle outline.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string" }
                },
                "required": ["x0", "y0", "x1", "y1", "color"]
            }),
        },
        ToolDefinition {
            name: "draw_filled_rect".into(),
            description: "Draw a filled rectangle.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string" }
                },
                "required": ["x0", "y0", "x1", "y1", "color"]
            }),
        },
        ToolDefinition {
            name: "draw_ellipse".into(),
            description: "Draw an ellipse outline within a bounding box.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string" }
                },
                "required": ["x0", "y0", "x1", "y1", "color"]
            }),
        },
        ToolDefinition {
            name: "draw_filled_ellipse".into(),
            description: "Draw a filled ellipse within a bounding box.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string" }
                },
                "required": ["x0", "y0", "x1", "y1", "color"]
            }),
        },
        ToolDefinition {
            name: "flood_fill".into(),
            description: "Flood fill a contiguous region starting from a point.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "color": { "type": "string" }
                },
                "required": ["x", "y", "color"]
            }),
        },
        ToolDefinition {
            name: "get_canvas_info".into(),
            description: "Get current canvas dimensions, layer info, and active layer.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "clear_canvas".into(),
            description: "Clear the active layer (fill with transparent).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "add_layer".into(),
            description: "Add a new layer with the given name and make it active.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Layer name" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "select_layer".into(),
            description: "Switch the active layer by index (0-based).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "description": "Layer index (0-based)" }
                },
                "required": ["index"]
            }),
        },
        ToolDefinition {
            name: "draw_filled_circle".into(),
            description: "Draw a filled circle given center and radius.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cx": { "type": "integer", "description": "Center X" },
                    "cy": { "type": "integer", "description": "Center Y" },
                    "radius": { "type": "integer", "description": "Radius in pixels" },
                    "color": { "type": "string", "description": "Hex color" }
                },
                "required": ["cx", "cy", "radius", "color"]
            }),
        },
        ToolDefinition {
            name: "draw_thick_line".into(),
            description: "Draw a line with thickness (brush size > 1px).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string", "description": "Hex color" },
                    "thickness": { "type": "integer", "description": "Line thickness in pixels (2-8)" }
                },
                "required": ["x0", "y0", "x1", "y1", "color", "thickness"]
            }),
        },
        ToolDefinition {
            name: "fill_dithered".into(),
            description: "Fill a rectangle with a two-color dither pattern for retro shading/gradients.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color1": { "type": "string", "description": "First hex color" },
                    "color2": { "type": "string", "description": "Second hex color" },
                    "pattern": { "type": "string", "enum": ["checker", "h_stripes", "v_stripes", "diag_stripes"], "description": "Dither pattern type" }
                },
                "required": ["x0", "y0", "x1", "y1", "color1", "color2", "pattern"]
            }),
        },
        ToolDefinition {
            name: "draw_outline".into(),
            description: "Auto-generate an outline around all non-transparent pixels on the active layer.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "color": { "type": "string", "description": "Outline hex color (e.g. #1a1a2e)" }
                },
                "required": ["color"]
            }),
        },
        ToolDefinition {
            name: "replace_color".into(),
            description: "Replace all pixels of one color with another on the active layer.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "old_color": { "type": "string", "description": "Color to replace (hex)" },
                    "new_color": { "type": "string", "description": "Replacement color (hex)" }
                },
                "required": ["old_color", "new_color"]
            }),
        },
        ToolDefinition {
            name: "gradient_fill".into(),
            description: "Fill a rectangle with a stepped vertical gradient between two colors.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color1": { "type": "string", "description": "Top color (hex)" },
                    "color2": { "type": "string", "description": "Bottom color (hex)" },
                    "steps": { "type": "integer", "description": "Number of color bands (2-16, e.g. 4 for sky)" }
                },
                "required": ["x0", "y0", "x1", "y1", "color1", "color2", "steps"]
            }),
        },
        ToolDefinition {
            name: "set_blend_mode".into(),
            description: "Set the blend mode of the active layer. Use with separate layers for shadow/highlight effects.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["normal", "multiply", "screen", "overlay"], "description": "Blend mode" }
                },
                "required": ["mode"]
            }),
        },
        ToolDefinition {
            name: "flip_horizontal".into(),
            description: "Flip the active layer horizontally (mirror left/right).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "flip_vertical".into(),
            description: "Flip the active layer vertically (mirror top/bottom).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "rotate_90".into(),
            description: "Rotate the active layer 90° clockwise (square canvas only).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "draw_filled_polygon".into(),
            description: "Draw a filled polygon from a list of vertices. Great for complex silhouettes, roofs, mountains, crystals.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "vertices": {
                        "type": "array",
                        "description": "Array of {x, y} vertex points forming the polygon",
                        "items": {
                            "type": "object",
                            "properties": {
                                "x": { "type": "integer" },
                                "y": { "type": "integer" }
                            },
                            "required": ["x", "y"]
                        }
                    },
                    "color": { "type": "string", "description": "Fill hex color" }
                },
                "required": ["vertices", "color"]
            }),
        },
        ToolDefinition {
            name: "spray_pixels".into(),
            description: "Scatter random pixels in a rectangle for textures (stars, grass, dirt, noise, particles).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "x0": { "type": "integer" },
                    "y0": { "type": "integer" },
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "color": { "type": "string", "description": "Hex color" },
                    "density": { "type": "integer", "description": "Percentage 1-100 (e.g. 10 = sparse stars, 50 = dense texture)" }
                },
                "required": ["x0", "y0", "x1", "y1", "color", "density"]
            }),
        },
        ToolDefinition {
            name: "copy_prev_frame".into(),
            description: "Copy the previous frame's content to the current frame. Use this at the start of each animation frame (except frame 0) to maintain consistency, then modify only what changes.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "finish".into(),
            description: "Call this when you are done drawing. This ends the agent loop.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Final message to user" }
                },
                "required": ["message"]
            }),
        },
    ]
}

// ── Tool execution ─────────────────────────────────────────────

/// Result of executing a single tool call.
pub struct ToolExecResult {
    pub output: String,
    pub is_error: bool,
    #[allow(dead_code)]
    pub finished: bool,
}

/// Execute a tool call on the canvas. Returns a text result for the API.
pub fn execute_tool(
    name: &str,
    input: &Value,
    canvas: &mut Canvas,
    cmd: &mut Command,
) -> ToolExecResult {
    match name {
        "set_pixels" => exec_set_pixels(input, canvas, cmd),
        "draw_line" => exec_draw_shape(name, input, canvas, cmd),
        "draw_rect" => exec_draw_shape(name, input, canvas, cmd),
        "draw_filled_rect" => exec_draw_shape(name, input, canvas, cmd),
        "draw_ellipse" => exec_draw_shape(name, input, canvas, cmd),
        "draw_filled_ellipse" => exec_draw_shape(name, input, canvas, cmd),
        "draw_filled_circle" => exec_filled_circle(input, canvas, cmd),
        "draw_thick_line" => exec_thick_line(input, canvas, cmd),
        "fill_dithered" => exec_fill_dithered(input, canvas, cmd),
        "draw_outline" => exec_draw_outline(input, canvas, cmd),
        "replace_color" => exec_replace_color(input, canvas, cmd),
        "gradient_fill" => exec_gradient_fill(input, canvas, cmd),
        "set_blend_mode" => exec_set_blend_mode(input, canvas),
        "flip_horizontal" => exec_flip_horizontal(canvas, cmd),
        "flip_vertical" => exec_flip_vertical(canvas, cmd),
        "rotate_90" => exec_rotate_90(canvas, cmd),
        "draw_filled_polygon" => exec_filled_polygon(input, canvas, cmd),
        "spray_pixels" => exec_spray_pixels(input, canvas, cmd),
        "flood_fill" => exec_flood_fill(input, canvas, cmd),
        "get_canvas_info" => exec_get_canvas_info(canvas),
        "clear_canvas" => exec_clear_canvas(canvas, cmd),
        "add_layer" => exec_add_layer(input, canvas),
        "select_layer" => exec_select_layer(input, canvas),
        "finish" => exec_finish(input),
        _ => ToolExecResult {
            output: format!("Unknown tool: {name}"),
            is_error: true,
            finished: false,
        },
    }
}

// ── Helpers ────────────────────────────────────────────────────

fn parse_color(hex: &str) -> Result<Color, String> {
    Color::from_hex(hex).ok_or_else(|| format!("Invalid color: {hex}"))
}

fn ok(msg: impl Into<String>) -> ToolExecResult {
    ToolExecResult {
        output: msg.into(),
        is_error: false,
        finished: false,
    }
}

fn err(msg: impl Into<String>) -> ToolExecResult {
    ToolExecResult {
        output: msg.into(),
        is_error: true,
        finished: false,
    }
}

fn exec_set_pixels(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let Some(pixels) = input.get("pixels").and_then(|v| v.as_array()) else {
        return err("Missing 'pixels' array");
    };
    let mut count = 0u32;
    for p in pixels {
        let Some(x) = p.get("x").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some(y) = p.get("y").and_then(|v| v.as_i64()) else {
            continue;
        };
        let Some(hex) = p.get("color").and_then(|v| v.as_str()) else {
            continue;
        };
        let color = match parse_color(hex) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let bx = canvas.to_buf_x(x as i32);
        let by = canvas.to_buf_y(y as i32);
        if bx >= 0 && by >= 0 {
            pxlot_tools::pencil_pixel(canvas, bx as u32, by as u32, color, cmd);
            count += 1;
        }
    }
    ok(format!("Set {count} pixels"))
}

fn exec_draw_shape(
    name: &str,
    input: &Value,
    canvas: &mut Canvas,
    cmd: &mut Command,
) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("#ffffff");
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };

    // Convert frame coords to buffer coords
    let bx0 = canvas.to_buf_x(x0);
    let by0 = canvas.to_buf_y(y0);
    let bx1 = canvas.to_buf_x(x1);
    let by1 = canvas.to_buf_y(y1);

    match name {
        "draw_line" => pxlot_tools::draw_line(canvas, bx0, by0, bx1, by1, color, cmd),
        "draw_rect" => pxlot_tools::draw_rect(canvas, bx0, by0, bx1, by1, color, cmd),
        "draw_filled_rect" => pxlot_tools::draw_filled_rect(canvas, bx0, by0, bx1, by1, color, cmd),
        "draw_ellipse" => pxlot_tools::draw_ellipse(canvas, bx0, by0, bx1, by1, color, cmd),
        "draw_filled_ellipse" => {
            pxlot_tools::draw_filled_ellipse(canvas, bx0, by0, bx1, by1, color, cmd)
        }
        _ => unreachable!(),
    }
    ok(format!("{name} OK"))
}

fn exec_flood_fill(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x = input.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y = input.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("#ffffff");
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let bx = canvas.to_buf_x(x);
    let by = canvas.to_buf_y(y);
    if bx < 0 || by < 0 {
        return err("Coordinates out of bounds");
    }
    let success = pxlot_tools::flood_fill(canvas, bx as u32, by as u32, color, cmd);
    if success {
        ok("flood_fill OK")
    } else {
        err("flood_fill failed (region too large or locked layer)")
    }
}

fn exec_get_canvas_info(canvas: &Canvas) -> ToolExecResult {
    let layers: Vec<Value> = canvas
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            json!({
                "index": i,
                "name": l.name,
                "visible": l.visible,
                "locked": l.locked,
                "opacity": l.opacity
            })
        })
        .collect();
    let info = json!({
        "width": canvas.frame_width(),
        "height": canvas.frame_height(),
        "active_layer": canvas.active_layer,
        "layers": layers
    });
    ok(info.to_string())
}

fn exec_clear_canvas(canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let layer_idx = canvas.active_layer;
    let fx0 = canvas.frame_x;
    let fy0 = canvas.frame_y;
    let fw = canvas.frame_width();
    let fh = canvas.frame_height();

    let Some(layer) = canvas.active_layer_mut() else {
        return err("No active layer");
    };
    if layer.locked || !layer.visible {
        return err("Layer is locked or hidden");
    }

    // Record changes and clear in one pass
    for fy in 0..fh {
        for fx in 0..fw {
            let bx = fx0 + fx;
            let by = fy0 + fy;
            if let Some(&old) = layer.buffer.get_pixel(bx, by) {
                if old != Color::TRANSPARENT {
                    cmd.add_change(layer_idx, bx, by, old, Color::TRANSPARENT);
                    layer.buffer.set_pixel(bx, by, Color::TRANSPARENT);
                }
            }
        }
    }
    ok("Canvas cleared")
}

fn exec_add_layer(input: &Value, canvas: &mut Canvas) -> ToolExecResult {
    let name = input
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("New Layer");
    match canvas.add_layer(name) {
        Some(idx) => ok(format!("Added layer '{name}' at index {idx}")),
        None => err("Cannot add layer (memory limit)"),
    }
}

fn exec_select_layer(input: &Value, canvas: &mut Canvas) -> ToolExecResult {
    let idx = input.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    if idx < canvas.layers.len() {
        canvas.active_layer = idx;
        ok(format!("Selected layer {idx}"))
    } else {
        err(format!(
            "Layer index {idx} out of range (0..{})",
            canvas.layers.len()
        ))
    }
}

fn exec_filled_circle(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let cx = input.get("cx").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let cy = input.get("cy").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let radius = input.get("radius").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let bcx = canvas.to_buf_x(cx);
    let bcy = canvas.to_buf_y(cy);
    pxlot_tools::draw_filled_circle(canvas, bcx, bcy, radius, color, cmd);
    ok("draw_filled_circle OK")
}

fn exec_thick_line(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let thickness = input.get("thickness").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let thickness = thickness.clamp(2, 8);
    let bx0 = canvas.to_buf_x(x0);
    let by0 = canvas.to_buf_y(y0);
    let bx1 = canvas.to_buf_x(x1);
    let by1 = canvas.to_buf_y(y1);
    pxlot_tools::draw_thick_line(canvas, bx0, by0, bx1, by1, color, thickness, cmd);
    ok("draw_thick_line OK")
}

fn exec_fill_dithered(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex1 = input.get("color1").and_then(|v| v.as_str()).unwrap_or("#000000");
    let hex2 = input.get("color2").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let pattern_str = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("checker");
    let color1 = match parse_color(hex1) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let color2 = match parse_color(hex2) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let pattern = match pattern_str {
        "h_stripes" => pxlot_tools::DitherPattern::HStripes,
        "v_stripes" => pxlot_tools::DitherPattern::VStripes,
        "diag_stripes" => pxlot_tools::DitherPattern::DiagStripes,
        _ => pxlot_tools::DitherPattern::Checker,
    };
    let bx0 = canvas.to_buf_x(x0);
    let by0 = canvas.to_buf_y(y0);
    let bx1 = canvas.to_buf_x(x1);
    let by1 = canvas.to_buf_y(y1);
    pxlot_tools::fill_dithered(canvas, bx0, by0, bx1, by1, color1, color2, pattern, cmd);
    ok("fill_dithered OK")
}

fn exec_draw_outline(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#1a1a2e");
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    pxlot_tools::draw_outline(canvas, color, cmd);
    ok("draw_outline OK")
}

fn exec_replace_color(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let old_hex = input.get("old_color").and_then(|v| v.as_str()).unwrap_or("#000000");
    let new_hex = input.get("new_color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let old_color = match parse_color(old_hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let new_color = match parse_color(new_hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    pxlot_tools::replace_color(canvas, old_color, new_color, cmd);
    ok("replace_color OK")
}

fn exec_gradient_fill(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex1 = input.get("color1").and_then(|v| v.as_str()).unwrap_or("#000000");
    let hex2 = input.get("color2").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let steps = input.get("steps").and_then(|v| v.as_u64()).unwrap_or(4) as u32;
    let color1 = match parse_color(hex1) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let color2 = match parse_color(hex2) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let bx0 = canvas.to_buf_x(x0);
    let by0 = canvas.to_buf_y(y0);
    let bx1 = canvas.to_buf_x(x1);
    let by1 = canvas.to_buf_y(y1);
    pxlot_tools::gradient_fill(canvas, bx0, by0, bx1, by1, color1, color2, steps, cmd);
    ok("gradient_fill OK")
}

fn exec_set_blend_mode(input: &Value, canvas: &mut Canvas) -> ToolExecResult {
    let mode_str = input.get("mode").and_then(|v| v.as_str()).unwrap_or("normal");
    let mode = match mode_str {
        "multiply" => pxlot_core::BlendMode::Multiply,
        "screen" => pxlot_core::BlendMode::Screen,
        "overlay" => pxlot_core::BlendMode::Overlay,
        _ => pxlot_core::BlendMode::Normal,
    };
    let idx = canvas.active_layer;
    if canvas.set_layer_blend_mode(idx, mode) {
        ok(format!("Blend mode set to {mode_str}"))
    } else {
        err("Failed to set blend mode")
    }
}

fn exec_flip_horizontal(canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    pxlot_tools::flip_horizontal(canvas, cmd);
    ok("flip_horizontal OK")
}

fn exec_flip_vertical(canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    pxlot_tools::flip_vertical(canvas, cmd);
    ok("flip_vertical OK")
}

fn exec_rotate_90(canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    if pxlot_tools::rotate_90(canvas, cmd) {
        ok("rotate_90 OK")
    } else {
        err("rotate_90 failed (canvas must be square)")
    }
}

fn exec_filled_polygon(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let Some(verts) = input.get("vertices").and_then(|v| v.as_array()) else {
        return err("Missing 'vertices' array");
    };
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let vertices: Vec<(i32, i32)> = verts
        .iter()
        .filter_map(|v| {
            let x = v.get("x").and_then(|x| x.as_i64())? as i32;
            let y = v.get("y").and_then(|y| y.as_i64())? as i32;
            // Convert frame coords to buffer coords
            Some((canvas.to_buf_x(x), canvas.to_buf_y(y)))
        })
        .collect();
    if vertices.len() < 3 {
        return err("Need at least 3 vertices");
    }
    pxlot_tools::draw_filled_polygon(canvas, &vertices, color, cmd);
    ok("draw_filled_polygon OK")
}

fn exec_spray_pixels(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
    let density = input.get("density").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
    let color = match parse_color(hex) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    let bx0 = canvas.to_buf_x(x0);
    let by0 = canvas.to_buf_y(y0);
    let bx1 = canvas.to_buf_x(x1);
    let by1 = canvas.to_buf_y(y1);
    // Use coordinates as seed for deterministic results
    let seed = (x0.wrapping_mul(31) ^ y0.wrapping_mul(17) ^ x1.wrapping_mul(13) ^ y1.wrapping_mul(7)) as u32;
    pxlot_tools::spray_pixels(canvas, bx0, by0, bx1, by1, color, density, seed, cmd);
    ok("spray_pixels OK")
}

fn exec_finish(input: &Value) -> ToolExecResult {
    let msg = input
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Done");
    ToolExecResult {
        output: msg.to_string(),
        is_error: false,
        finished: true,
    }
}
