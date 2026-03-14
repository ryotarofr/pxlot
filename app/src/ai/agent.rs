/// Plan→Execute agent: asks the LLM for a JSON drawing plan, then executes it locally.
///
/// Supports both single-frame and multi-frame (animation) plans.
/// The model draws in a normalized 64x64 coordinate space (0–63).
/// Coordinates are clamped and offset to the canvas centre before execution.
use leptos::prelude::*;
use pxlot_core::history::Command;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::ai::ChatMessage;
use crate::ai::api_client::{self, ApiMessage, ContentBlock, MessagesRequest};
use crate::ai::tools;
use crate::state::EditorState;

/// Maximum operations to execute per frame.
const MAX_OPS_PER_FRAME: usize = 40;

/// Maximum frames in an animation plan.
const MAX_FRAMES: usize = 8;

/// The fixed drawing size the model works in.
const DRAW_SIZE: u32 = 64;

// ── System prompt ────────────────────────────────────────────

fn system_prompt() -> String {
    let max = DRAW_SIZE - 1;
    format!(
        r##"You are an expert pixel art assistant. You draw on a 64x64 pixel canvas.
Coordinates: (0,0) top-left, x right, y down. Valid range: x[0,{max}], y[0,{max}].

Respond with ONLY a valid JSON object. No markdown, no explanation.

You support TWO formats:

FORMAT 1 — Single image:
{{"description":"...","palette":{{"bg":"#hex","main":"#hex","dark":"#hex","light":"#hex","outline":"#hex"}},"operations":[{{"tool":"name",...}}]}}

FORMAT 2 — Animation (use when user asks for animation, movement, walking, idle, etc.):
{{"description":"...","fps":8,"palette":{{...}},"frames":[{{"operations":[...]}},{{"operations":[...]}}]}}

"palette": declare 4-8 named colors before drawing. Every color used in operations MUST come from this palette.

Tools:
- draw_filled_rect: x0,y0,x1,y1,color — filled rectangle
- draw_filled_ellipse: x0,y0,x1,y1,color — filled ellipse
- draw_rect: x0,y0,x1,y1,color — rectangle outline
- draw_ellipse: x0,y0,x1,y1,color — ellipse outline
- draw_line: x0,y0,x1,y1,color — straight line
- flood_fill: x,y,color — fill contiguous region
- set_pixels: pixels:[{{"x":int,"y":int,"color":"hex"}}] — ONLY for tiny details

RULES:
1. ALL coordinates MUST be in [0,{max}]. The canvas is exactly 64x64.
2. Order: background fill → large body → shading → outlines → tiny details (set_pixels last).
3. Use 2-3 shades for depth (base, shadow, highlight). Dark outlines (#1a1a2e) make shapes pop.
4. set_pixels ONLY for accents (<10 pixels). Never for fills.
5. Plan 10-25 operations per frame. Never use clear_canvas.
6. Combine overlapping shapes for complex silhouettes (e.g. teardrop = large ellipse + small ellipse on top).
7. For modifications: output only NEW operations to layer on top.

ANIMATION RULES (Format 2 only):
- Each frame is a COMPLETE drawing (not a delta). Redraw everything per frame.
- Keep character size, position, and palette consistent across frames.
- Animate with small changes (1-3px shifts per frame) for smooth motion.
- Frame count: idle=2-3, walk=4, attack=3-4, bounce=3-4.
- All frames share the SAME palette.

EXAMPLE — red mushroom (single image):
{{"description":"Red mushroom with white spots","palette":{{"bg":"#87ceeb","cap":"#cc2222","cap_dark":"#991111","stem":"#f5e6c8","stem_dark":"#d4c4a0","spot":"#ffffff","outline":"#1a1a2e"}},"operations":[{{"tool":"draw_filled_rect","x0":0,"y0":0,"x1":63,"y1":63,"color":"#87ceeb"}},{{"tool":"draw_filled_ellipse","x0":8,"y0":8,"x1":56,"y1":38,"color":"#cc2222"}},{{"tool":"draw_filled_ellipse","x0":10,"y0":14,"x1":34,"y1":36,"color":"#991111"}},{{"tool":"draw_filled_rect","x0":22,"y0":34,"x1":42,"y1":58,"color":"#f5e6c8"}},{{"tool":"draw_filled_rect","x0":24,"y0":34,"x1":40,"y1":40,"color":"#d4c4a0"}},{{"tool":"draw_filled_ellipse","x0":18,"y0":14,"x1":28,"y1":24,"color":"#ffffff"}},{{"tool":"draw_filled_ellipse","x0":36,"y0":18,"x1":44,"y1":26,"color":"#ffffff"}},{{"tool":"draw_ellipse","x0":8,"y0":8,"x1":56,"y1":38,"color":"#1a1a2e"}},{{"tool":"draw_rect","x0":22,"y0":34,"x1":42,"y1":58,"color":"#1a1a2e"}},{{"tool":"draw_line","x0":22,"y0":34,"x1":8,"y1":34,"color":"#1a1a2e"}},{{"tool":"draw_line","x0":42,"y0":34,"x1":56,"y1":34,"color":"#1a1a2e"}},{{"tool":"set_pixels","pixels":[{{"x":28,"y":46,"color":"#1a1a2e"}},{{"x":36,"y":46,"color":"#1a1a2e"}}]}}]}}"##
    )
}

// ── Types ────────────────────────────────────────────────────

/// Shared flag to signal the agent to stop.
pub type StopFlag = Arc<AtomicBool>;

/// Create a new stop flag.
pub fn new_stop_flag() -> StopFlag {
    Arc::new(AtomicBool::new(false))
}

/// Drawing plan parsed from LLM response — supports single-frame and animation.
#[derive(Deserialize)]
struct DrawingPlan {
    #[serde(default)]
    description: String,
    /// Single-frame operations (Format 1)
    #[serde(default)]
    operations: Vec<Value>,
    /// Animation frames (Format 2)
    #[serde(default)]
    frames: Vec<FramePlan>,
    /// FPS for animation (default 8)
    #[serde(default = "default_fps")]
    fps: u32,
}

fn default_fps() -> u32 {
    8
}

#[derive(Deserialize)]
struct FramePlan {
    #[serde(default)]
    operations: Vec<Value>,
}

impl DrawingPlan {
    /// Normalize plan into a list of frame operations.
    /// Format 1 (operations) → 1 frame. Format 2 (frames) → multiple frames.
    fn into_frames(self) -> (Vec<Vec<Value>>, u32) {
        if !self.frames.is_empty() {
            let frames: Vec<Vec<Value>> = self
                .frames
                .into_iter()
                .take(MAX_FRAMES)
                .map(|f| f.operations)
                .collect();
            (frames, self.fps.clamp(1, 30))
        } else {
            (vec![self.operations], 0) // fps=0 means single frame
        }
    }
}

// ── Agent entry point ────────────────────────────────────────

/// Run the agent: send a single API call to get a JSON drawing plan, then execute it.
pub async fn run_agent(
    user_text: String,
    model: String,
    editor: StoredValue<EditorState>,
    conversation: StoredValue<Vec<ApiMessage>>,
    set_messages: WriteSignal<Vec<ChatMessage>>,
    set_running: WriteSignal<bool>,
    set_token_usage: WriteSignal<(usize, usize)>,
    set_render_trigger: WriteSignal<u32>,
    stop_flag: StopFlag,
) {
    set_running.set(true);

    // Compute offset to centre the 64x64 drawing area on the real canvas
    let (offset_x, offset_y) = editor.with_value(|s| {
        let w = s.canvas.frame_width();
        let h = s.canvas.frame_height();
        (
            (w.saturating_sub(DRAW_SIZE) / 2) as i64,
            (h.saturating_sub(DRAW_SIZE) / 2) as i64,
        )
    });

    let system = system_prompt();

    // Add user message — no canvas image (saves massive tokens)
    conversation.update_value(|msgs| {
        msgs.push(ApiMessage {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: user_text.clone(),
            }],
        });
    });

    // Single API call with no tool definitions — just ask for JSON plan
    let api_messages = conversation.with_value(|msgs| msgs.clone());
    let request = MessagesRequest {
        model,
        max_tokens: 8192,
        system,
        messages: api_messages,
        tools: vec![],
    };

    add_status(&set_messages, "Generating drawing plan...");

    let response = match api_client::send_message(&request).await {
        Ok(r) => r,
        Err(e) => {
            add_status(&set_messages, &format!("API error: {e}"));
            set_running.set(false);
            return;
        }
    };

    // Update token usage
    if let Some(usage) = &response.usage {
        set_token_usage.set((usage.input_tokens, usage.output_tokens));
    }

    // Extract text from response
    let response_text: String = response
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    // Compress assistant response before saving to conversation history.
    // Store only a short summary instead of the full JSON plan to save tokens
    // on subsequent requests. The last plan is kept in full for follow-up accuracy.
    compress_history(&conversation);
    conversation.update_value(|msgs| {
        msgs.push(ApiMessage {
            role: "assistant".into(),
            content: response.content.clone(),
        });
    });

    // Parse drawing plan from JSON
    let plan = match parse_plan(&response_text) {
        Ok(p) => p,
        Err(e) => {
            add_status(&set_messages, &format!("Plan parse error: {e}"));
            set_messages.update(|msgs| {
                msgs.push(ChatMessage::assistant(&response_text));
            });
            set_running.set(false);
            return;
        }
    };

    // Show plan description
    if !plan.description.is_empty() {
        set_messages.update(|msgs| {
            msgs.push(ChatMessage::assistant(&plan.description));
        });
    }

    let (frames, fps) = plan.into_frames();
    let is_animation = frames.len() > 1;

    if is_animation {
        add_status(
            &set_messages,
            &format!("Animation: {} frames at {} FPS", frames.len(), fps),
        );
    }

    // Execute each frame
    let mut total_executed = 0u32;

    for (frame_idx, frame_ops) in frames.iter().enumerate() {
        if stop_flag.load(Ordering::Relaxed) {
            add_status(&set_messages, "Stopped by user.");
            break;
        }

        // For animation: add new frame (frame 0 uses the existing current frame)
        if is_animation && frame_idx > 0 {
            editor.update_value(|state| {
                state.add_frame();
            });
        }

        if is_animation {
            add_status(
                &set_messages,
                &format!("Drawing frame {}/{}...", frame_idx + 1, frames.len()),
            );
        }

        // Execute operations for this frame
        let ops = &frame_ops[..frame_ops.len().min(MAX_OPS_PER_FRAME)];
        let executed =
            execute_frame_ops(ops, offset_x, offset_y, &editor, &set_messages, &stop_flag);
        total_executed += executed;

        // Save frame to timeline
        if is_animation {
            editor.update_value(|state| {
                state.save_frame();
            });
        }
    }

    // Set FPS for animations
    if is_animation && fps > 0 {
        editor.update_value(|state| {
            state.timeline.fps = fps;
            // Go back to first frame for preview
            state.switch_frame(0);
        });
    }

    // Single render trigger after all frames
    if total_executed > 0 {
        set_render_trigger.update(|v| *v += 1);
    }

    let summary = if is_animation {
        format!(
            "Done — {} frames, {} operations total.",
            frames.len(),
            total_executed
        )
    } else {
        format!("Done — {total_executed} operations executed.")
    };
    add_status(&set_messages, &summary);
    set_running.set(false);
}

// ── Frame execution ──────────────────────────────────────────

/// Execute a list of operations on the current canvas frame.
/// Returns the number of operations executed.
fn execute_frame_ops(
    ops: &[Value],
    offset_x: i64,
    offset_y: i64,
    editor: &StoredValue<EditorState>,
    set_messages: &WriteSignal<Vec<ChatMessage>>,
    stop_flag: &StopFlag,
) -> u32 {
    let mut executed = 0u32;

    for op in ops {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        let tool_name = match op.get("tool").and_then(|v| v.as_str()) {
            Some(n) if n != "finish" && n != "clear_canvas" => n,
            _ => continue,
        };

        // Clamp + offset coordinates from 64x64 space to real canvas
        let offset_op = offset_coordinates(op, offset_x, offset_y);

        // Show tool in chat
        set_messages.update(|msgs| {
            msgs.push(ChatMessage::tool(tool_name, crate::ai::ToolStatus::Running));
        });

        // Execute on canvas
        let mut is_error = false;
        let mut output = String::new();

        editor.update_value(|state| {
            let mut cmd = Command::new(format!("ai:{tool_name}"));
            let result = tools::execute_tool(tool_name, &offset_op, &mut state.canvas, &mut cmd);
            if !cmd.is_empty() {
                state.history.push(cmd);
            }
            is_error = result.is_error;
            output = result.output;
        });

        // Update tool status
        let name_s = tool_name.to_string();
        let out_s = output;
        set_messages.update(|msgs| {
            if let Some(last) = msgs.last_mut() {
                last.content = if is_error {
                    crate::ai::ChatContent::ToolUse {
                        name: name_s,
                        status: crate::ai::ToolStatus::Error(out_s),
                    }
                } else {
                    crate::ai::ChatContent::ToolUse {
                        name: name_s,
                        status: crate::ai::ToolStatus::Done,
                    }
                };
            }
        });

        executed += 1;
    }

    executed
}

// ── Conversation history compression ─────────────────────────

/// Compress older assistant responses in the conversation history.
/// Keeps the most recent assistant response in full (for follow-up accuracy).
/// Replaces all older assistant responses with a short text summary.
fn compress_history(conversation: &StoredValue<Vec<ApiMessage>>) {
    conversation.update_value(|msgs| {
        // Find indices of all assistant messages
        let assistant_indices: Vec<usize> = msgs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == "assistant")
            .map(|(i, _)| i)
            .collect();

        // Only compress if there are 2+ assistant messages (keep the latest intact)
        if assistant_indices.len() < 2 {
            return;
        }

        // Compress all but the last assistant message
        let to_compress = &assistant_indices[..assistant_indices.len() - 1];
        for &idx in to_compress {
            // Extract a short summary from the existing content
            let summary = extract_summary(&msgs[idx].content);
            msgs[idx].content = vec![ContentBlock::Text { text: summary }];
        }
    });
}

/// Extract a short text summary from content blocks.
fn extract_summary(content: &[ContentBlock]) -> String {
    for block in content {
        if let ContentBlock::Text { text } = block {
            // Try to extract the "description" field from JSON
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                if let Some(desc) = v.get("description").and_then(|d| d.as_str()) {
                    let frame_count = v
                        .get("frames")
                        .and_then(|f| f.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    if frame_count > 0 {
                        return format!("[Previous: {desc} ({frame_count} frames)]");
                    }
                    return format!("[Previous: {desc}]");
                }
            }
            // Fallback: truncate raw text
            let truncated: String = text.chars().take(80).collect();
            return format!("[Previous response: {truncated}...]");
        }
    }
    "[Previous drawing]".to_string()
}

// ── Coordinate offset ────────────────────────────────────────

/// Clamp a coordinate to the [0, DRAW_SIZE-1] range, then add the canvas offset.
fn clamp_and_offset(val: i64, offset: i64) -> i64 {
    let max = (DRAW_SIZE - 1) as i64;
    val.clamp(0, max) + offset
}

/// Clamp all coordinates to the 64x64 drawing space, then offset to the real canvas position.
/// This guarantees the drawing stays within 64x64 regardless of what the model outputs.
fn offset_coordinates(op: &Value, dx: i64, dy: i64) -> Value {
    let mut out = op.clone();

    let coord_keys_xy = [("x0", "y0"), ("x1", "y1")];
    for (xk, yk) in &coord_keys_xy {
        if let Some(x) = op.get(xk).and_then(|v| v.as_i64()) {
            out[xk] = json!(clamp_and_offset(x, dx));
        }
        if let Some(y) = op.get(yk).and_then(|v| v.as_i64()) {
            out[yk] = json!(clamp_and_offset(y, dy));
        }
    }

    // Single-point tools (flood_fill)
    if let Some(x) = op.get("x").and_then(|v| v.as_i64()) {
        out["x"] = json!(clamp_and_offset(x, dx));
    }
    if let Some(y) = op.get("y").and_then(|v| v.as_i64()) {
        out["y"] = json!(clamp_and_offset(y, dy));
    }

    // set_pixels — clamp and offset each pixel
    if let Some(pixels) = op.get("pixels").and_then(|v| v.as_array()) {
        let offset_pixels: Vec<Value> = pixels
            .iter()
            .map(|p| {
                let mut pp = p.clone();
                if let Some(x) = p.get("x").and_then(|v| v.as_i64()) {
                    pp["x"] = json!(clamp_and_offset(x, dx));
                }
                if let Some(y) = p.get("y").and_then(|v| v.as_i64()) {
                    pp["y"] = json!(clamp_and_offset(y, dy));
                }
                pp
            })
            .collect();
        out["pixels"] = json!(offset_pixels);
    }

    out
}

// ── JSON parsing ─────────────────────────────────────────────

/// Parse a drawing plan from LLM response text.
fn parse_plan(text: &str) -> Result<DrawingPlan, String> {
    let json_str = extract_json(text)?;
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

/// Extract a JSON object from text, handling possible markdown wrapping.
fn extract_json(text: &str) -> Result<&str, String> {
    let t = text.trim();

    // Direct JSON
    if t.starts_with('{') {
        return Ok(t);
    }

    // JSON in code fence
    if let Some(fence) = t.find("```") {
        let after = &t[fence + 3..];
        let start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        if let Some(end) = after[start..].find("```") {
            return Ok(after[start..start + end].trim());
        }
    }

    // Outermost braces
    if let Some(s) = t.find('{') {
        if let Some(e) = t.rfind('}') {
            if e > s {
                return Ok(&t[s..=e]);
            }
        }
    }

    Err("No JSON found in response".into())
}

/// Helper to add a status message to the chat.
fn add_status(set_messages: &WriteSignal<Vec<ChatMessage>>, text: &str) {
    set_messages.update(|msgs| {
        msgs.push(ChatMessage::status(text));
    });
}
