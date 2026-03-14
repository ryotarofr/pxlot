/// Tool definitions and execution bridge for the AI agent.
/// Maps LLM tool_use calls to pxlot_tools / Canvas operations.
use pxlot_core::history::Command;
use pxlot_core::{Canvas, Color};
use serde_json::{json, Value};

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

fn exec_draw_shape(name: &str, input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x0 = input.get("x0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y0 = input.get("y0").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x1 = input.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y1 = input.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
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
        "draw_filled_ellipse" => pxlot_tools::draw_filled_ellipse(canvas, bx0, by0, bx1, by1, color, cmd),
        _ => unreachable!(),
    }
    ok(format!("{name} OK"))
}

fn exec_flood_fill(input: &Value, canvas: &mut Canvas, cmd: &mut Command) -> ToolExecResult {
    let x = input.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y = input.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let hex = input.get("color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
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
