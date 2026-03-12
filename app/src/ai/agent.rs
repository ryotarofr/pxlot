/// Agent loop: orchestrates the conversation between user, LLM, and canvas tools.
use leptos::prelude::*;
use pxlot_core::history::Command;
use pxlot_formats::png_format;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ai::api_client::{self, ApiMessage, ContentBlock, ImageSource, MessagesRequest};
use crate::ai::tools;
use crate::ai::ChatMessage;
use crate::state::EditorState;

/// How often to send a canvas screenshot to the LLM (every N turns).
const SEND_IMAGE_EVERY: usize = 3;

/// Maximum turns before forcefully stopping.
const MAX_TURNS: usize = 25;

/// System prompt template.
fn system_prompt(width: u32, height: u32) -> String {
    let xmax = width.saturating_sub(1);
    let ymax = height.saturating_sub(1);
    format!(
        r##"You are a pixel art drawing assistant. You have a {width}x{height} pixel canvas.

Coordinate system:
- (0,0) is the top-left corner
- x increases rightward, y increases downward
- Valid range: x in [0, {xmax}], y in [0, {ymax}]

Colors are specified as hex strings like "#ff0000" (red).

Available tools:
- set_pixels: Set individual pixels (batch multiple in one call for efficiency)
- draw_line: Draw a line between two points
- draw_rect / draw_filled_rect: Draw rectangle outline or filled
- draw_ellipse / draw_filled_ellipse: Draw ellipse outline or filled
- flood_fill: Fill a contiguous region
- get_canvas_info: Get canvas dimensions and layer info
- clear_canvas: Clear the active layer
- add_layer: Add a new layer
- select_layer: Switch active layer
- finish: Call when done drawing (required to end)

Guidelines:
- Work methodically: plan the drawing, then execute step by step
- Use layers strategically (e.g. background, outline, detail)
- Batch pixels in set_pixels when setting many individual pixels
- Use shape tools (draw_rect, draw_filled_rect, draw_ellipse) for geometric shapes
- Use flood_fill for large areas of solid color
- Call finish when the drawing is complete
- Keep pixel art style: limited colors, clean shapes, intentional pixel placement"##
    )
}

/// Shared flag to signal the agent to stop.
pub type StopFlag = Arc<AtomicBool>;

/// Create a new stop flag.
pub fn new_stop_flag() -> StopFlag {
    Arc::new(AtomicBool::new(false))
}

/// Run the agent loop asynchronously.
///
/// This function is `spawn_local`'d from the UI callback.
/// It communicates progress back to the UI via signal setters.
///
/// `conversation` holds the API message history (persisted across sends).
pub async fn run_agent(
    user_text: String,
    api_key: String,
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

    // Get canvas dimensions
    let (width, height) = editor.with_value(|s| {
        (s.canvas.frame_width(), s.canvas.frame_height())
    });

    let system = system_prompt(width, height);
    let tool_defs = tools::tool_definitions();

    // Add user message + canvas image to conversation
    let mut user_content = vec![ContentBlock::Text {
        text: user_text.clone(),
    }];
    if let Some(image_block) = capture_canvas_image(editor) {
        user_content.push(image_block);
    }
    conversation.update_value(|msgs| {
        msgs.push(ApiMessage {
            role: "user".into(),
            content: user_content,
        });
    });

    let mut total_input = 0usize;
    let mut total_output = 0usize;

    for turn in 0..MAX_TURNS {
        if stop_flag.load(Ordering::Relaxed) {
            add_status(&set_messages, "Stopped by user.");
            break;
        }

        // Build API request from full conversation history
        let api_messages = conversation.with_value(|msgs| msgs.clone());
        let request = MessagesRequest {
            model: model.clone(),
            max_tokens: 4096,
            system: system.clone(),
            messages: api_messages,
            tools: tool_defs.clone(),
        };

        let response = match api_client::send_message(&api_key, &request).await {
            Ok(r) => r,
            Err(e) => {
                add_status(&set_messages, &format!("API error: {e}"));
                break;
            }
        };

        // Update token usage
        if let Some(usage) = &response.usage {
            total_input += usage.input_tokens;
            total_output += usage.output_tokens;
            set_token_usage.set((total_input, total_output));
        }

        // Process response content blocks
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        let mut tool_results: Vec<ContentBlock> = Vec::new();
        let mut finished = false;

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    set_messages.update(|msgs| {
                        msgs.push(ChatMessage::assistant(text));
                    });
                    assistant_content.push(block.clone());
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // Show tool running
                    set_messages.update(|msgs| {
                        msgs.push(ChatMessage::tool(
                            name,
                            crate::ai::ToolStatus::Running,
                        ));
                    });

                    // Execute tool on canvas
                    let mut tool_output = String::new();
                    let mut tool_is_error = false;
                    let mut tool_finished = false;

                    editor.update_value(|state| {
                        let mut cmd = Command::new(format!("ai:{name}"));
                        let result = tools::execute_tool(name, input, &mut state.canvas, &mut cmd);
                        if !cmd.is_empty() {
                            state.history.push(cmd);
                        }
                        tool_output = result.output;
                        tool_is_error = result.is_error;
                        tool_finished = result.finished;
                    });

                    // Update tool status in chat
                    let name_clone = name.clone();
                    let output_clone = tool_output.clone();
                    set_messages.update(|msgs| {
                        if let Some(last) = msgs.last_mut() {
                            last.content = if tool_is_error {
                                crate::ai::ChatContent::ToolUse {
                                    name: name_clone,
                                    status: crate::ai::ToolStatus::Error(output_clone),
                                }
                            } else {
                                crate::ai::ChatContent::ToolUse {
                                    name: name_clone,
                                    status: crate::ai::ToolStatus::Done,
                                }
                            };
                        }
                    });

                    if tool_finished {
                        finished = true;
                        let output_msg = tool_output.clone();
                        set_messages.update(|msgs| {
                            msgs.push(ChatMessage::assistant(&output_msg));
                        });
                    }

                    assistant_content.push(block.clone());
                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: tool_output,
                        is_error: if tool_is_error { Some(true) } else { None },
                    });
                }
                _ => {
                    assistant_content.push(block.clone());
                }
            }
        }

        // Batch re-render: trigger once after all tools in this turn
        if !tool_results.is_empty() {
            set_render_trigger.update(|v| *v += 1);
        }

        // Append assistant response to conversation history
        conversation.update_value(|msgs| {
            msgs.push(ApiMessage {
                role: "assistant".into(),
                content: assistant_content,
            });
        });

        if finished {
            break;
        }

        // Send tool results back
        if !tool_results.is_empty() {
            // Periodically attach canvas image for visual feedback
            if (turn + 1) % SEND_IMAGE_EVERY == 0 {
                if let Some(image_block) = capture_canvas_image(editor) {
                    tool_results.push(image_block);
                }
            }

            conversation.update_value(|msgs| {
                msgs.push(ApiMessage {
                    role: "user".into(),
                    content: tool_results,
                });
            });
        } else {
            // No tool calls — model sent only text.
            if response.stop_reason.as_deref() == Some("end_turn") {
                break;
            }
        }
    }

    set_running.set(false);
}

/// Capture the current canvas as a base64 PNG image content block.
fn capture_canvas_image(editor: StoredValue<EditorState>) -> Option<ContentBlock> {
    let png_bytes = editor.with_value(|state| png_format::export_png(&state.canvas).ok())?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Some(ContentBlock::Image {
        source: ImageSource {
            source_type: "base64".into(),
            media_type: "image/png".into(),
            data: b64,
        },
    })
}

/// Helper to add a status message to the chat.
fn add_status(set_messages: &WriteSignal<Vec<ChatMessage>>, text: &str) {
    set_messages.update(|msgs| {
        msgs.push(ChatMessage::status(text));
    });
}
