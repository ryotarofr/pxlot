/// Plan→Execute agent: asks the LLM for a JSON drawing plan, then executes it locally.
///
/// Supports both single-frame and multi-frame (animation) plans.
/// The model draws in a normalized 64x64 coordinate space (0–63).
/// Coordinates are clamped and offset to the canvas centre before execution.
use leptos::prelude::*;
use pxlot_core::history::Command;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use crate::ai::ChatMessage;
use crate::ai::api_client::{self, ApiMessage, ContentBlock, ImageSource, MessagesRequest};
use crate::ai::tools;
use crate::state::EditorState;

/// Maximum operations to execute per frame.
const MAX_OPS_PER_FRAME: usize = 150;

/// Maximum frames in an animation plan.
const MAX_FRAMES: usize = 8;

/// The fixed drawing size the model works in.
const DRAW_SIZE: u32 = 64;

// ── Drawing phases ───────────────────────────────────────────

/// A drawing phase for the multi-turn iterative approach.
struct DrawingPhase {
    name: &'static str,
    /// Whether this phase receives a canvas image as input
    needs_image: bool,
    /// Whether to use the cheaper model (Sonnet) for this phase
    use_cheap_model: bool,
    /// Max tokens for this phase's response
    max_tokens: usize,
}

/// The 4 drawing phases for single-frame generation.
const PHASES: &[DrawingPhase] = &[
    DrawingPhase {
        name: "silhouette",
        needs_image: false,
        use_cheap_model: false,
        max_tokens: 4096,
    },
    DrawingPhase {
        name: "color_and_form",
        needs_image: true,
        use_cheap_model: false,
        max_tokens: 6144,
    },
    DrawingPhase {
        name: "shading_and_outline",
        needs_image: true,
        use_cheap_model: true,
        max_tokens: 6144,
    },
    DrawingPhase {
        name: "detail_and_polish",
        needs_image: true,
        use_cheap_model: true,
        max_tokens: 4096,
    },
];

/// Cheaper model used for later drawing phases.
const CHEAP_MODEL: &str = "claude-sonnet-4-6";

// ── System prompt ────────────────────────────────────────────

fn base_system_intro() -> String {
    let max = DRAW_SIZE - 1;
    format!(
        r##"You are a professional pixel art artist creating high-quality 64x64 sprite art.
Coordinates: (0,0) top-left, x right, y down. Range: x[0,{max}], y[0,{max}].
Respond with ONLY valid JSON: {{"description":"...","palette":{{...}},"operations":[...]}}

PALETTE: 12-20 named colors with 3-4 shades per area. Outlines = dark saturated hues, NOT #000000.
Example: {{"body":"#cc5544","body_dk":"#882233","body_lt":"#dd7766","body_ol":"#551122",...}}
COLOR REFERENCING: Use palette KEY NAMES in color fields. "body" resolves to "#cc5544".

TOOLS:
- draw_filled_rect: x0,y0,x1,y1,color
- draw_filled_ellipse: x0,y0,x1,y1,color
- draw_filled_circle: cx,cy,radius,color
- draw_filled_polygon: vertices:[{{"x":int,"y":int}}],color — ESSENTIAL for organic shapes
- draw_rect/draw_ellipse: x0,y0,x1,y1,color (outlines)
- draw_line/draw_thick_line: x0,y0,x1,y1,color[,thickness]
- flood_fill: x,y,color
- set_pixels: {{"tool":"set_pixels","color":"key","xy":"x,y;x,y;x,y"}} (compact single-color)
  Multi-color: {{"tool":"set_pixels","groups":[{{"color":"key","xy":"x,y;x,y"}}]}}
- spray_pixels: x0,y0,x1,y1,color,density(1-100)
- fill_dithered: x0,y0,x1,y1,color1,color2,pattern
- gradient_fill: x0,y0,x1,y1,color1,color2,steps
- replace_color: old_color,new_color

RULES:
- Subject fits within canvas with 3-5px padding on all sides (50-70% fill).
- SIDE VIEW pose for maximum readability. Full body visible.
- NEVER use spray_pixels or fill_dithered on subject's body. Only for background effects.
- Surfaces = CLEAN smooth fills. No scattered dots or checker patterns on subject.
- Use compact set_pixels format."##
    )
}

/// Phase-specific system prompt for multi-turn drawing.
fn phase_system_prompt(phase_name: &str) -> String {
    let intro = base_system_intro();
    let phase_instructions = match phase_name {
        "silhouette" => r##"
YOUR TASK: PHASE 1 — SILHOUETTE & PALETTE DESIGN

You are starting from a BLANK canvas. This is the foundation — get the shapes right.

1. Define the COMPLETE palette in the "palette" field (12-20 colors, 3-4 shades per area).
2. Fill the background with a solid color (1 op).
3. Construct the subject's SILHOUETTE using MULTIPLE draw_filled_polygon ops — one per major body part.
   - Character: head, torso, each arm, each leg, hair/hat, weapon
   - Creature: head, body, each wing/limb, tail, horns
4. Use ONE silhouette color for ALL parts (the darkest body color).
5. Each polygon must have PRECISE vertices defining CLEAN, SHARP shapes.
6. TEST: filled with one solid color, would a viewer instantly recognize the subject? If not, refine.

Target: 5-12 operations. Focus on SHAPE ACCURACY, not color yet."##,

        "color_and_form" => r##"
YOUR TASK: PHASE 2 — BASE COLORS & FORM

You can SEE the current canvas (silhouette from Phase 1). Now add color and structure.
REUSE the same palette keys defined in Phase 1.

1. Layer distinct base colors per body part using draw_filled_polygon ON TOP of the silhouette.
   - Each body part gets its own color. Overlap freely — later shapes cover earlier ones.
2. Add sub-structures: wing membranes between bones, belly/chest area, face features.
3. Every area should have its BASE color applied — no silhouette color should remain visible
   except where it serves as an outline.

Target: 10-20 operations. Focus on COLOR SEPARATION between body parts."##,

        "shading_and_outline" => r##"
YOUR TASK: PHASE 3 — SHADING & OUTLINES

You can SEE the current canvas (colored form from Phase 2). Now add depth and definition.
REUSE the same palette keys.

SHADING (light from TOP-LEFT):
- Bottom-right edges of each body part: _dk shade via draw_filled_polygon to carve shadow shapes
- Top-left edges: _lt shade via set_pixels for highlight strips (1-2px wide)
- GRADUATED shading: base → mid → dark. Not just base → dark.
- Shade INSIDE each body part. Wings: lighter near bone, darker at tips.

OUTLINES:
- Draw outlines with set_pixels (NOT draw_rect/draw_ellipse).
- Use darkest shade of each region as outline color. NEVER #000000.
- Add inner structural lines: wing bones, scale rows, armor plates, cloth folds.
- Outlines define INTERNAL structure, not just external edges.

Target: 20-40 operations. set_pixels ≥50% of ops."##,

        "detail_and_polish" => r##"
YOUR TASK: PHASE 4 — DETAILS & POLISH

You can SEE the current canvas (shaded + outlined from Phase 3). Final quality pass.

1. ANTI-ALIASING: At diagonal edges, place mid-tone pixels to smooth staircases.
2. Eyes: 2-3px with 1px white highlight dot.
3. Specular highlights: 1-2 bright pixels on eyes, metal, gems.
4. Wing bones: ensure 2-3 bone lines visible with solid color bands between.
5. Fix any issues: orphan pixels, broken curves, messy edges.
6. Ensure clean surfaces — remove any scattered dots or noise.

If the art already looks good: {{"description":"no changes needed","palette":{{}},"operations":[]}}

Target: 10-30 operations. Precision fixes only, NOT large redraws."##,

        _ => "",
    };
    format!("{intro}\n{phase_instructions}")
}

/// System prompt for single-shot animation (kept separate from multi-turn phases).
fn animation_system_prompt() -> String {
    let intro = base_system_intro();
    format!(
        r##"{intro}

FORMAT: {{"description":"...","fps":8,"palette":{{...}},"frames":[{{"operations":[...]}}]}}

TECHNIQUE — follow this order within each frame:
1. SILHOUETTE: draw_filled_polygon per body part (4-8 ops)
2. BASE COLORS: layer colors per part (5-15 ops)
3. SHADING: _dk/_lt shades, light from top-left (10-25 ops)
4. OUTLINES: set_pixels, colored not black (8-20 ops)
5. DETAILS: anti-aliasing, eyes, highlights (15-35 ops)

60-120 ops per frame. set_pixels ≥40%.
3-4 shading levels per area.

ANIMATION:
- copy_prev_frame as FIRST op of frames 1+ to copy previous frame.
- Then apply ONLY the changes for that frame. Do NOT redraw entire subject.
- Animate by MOVING specific body parts (wings, legs, arms) with small shifts (2-4px per frame).
- Keep torso/body FIXED. Only limbs and accessories move.
- Frame count: idle=2-3, walk=4, attack=3-4, fly=3-4.
- Each frame must maintain the same overall silhouette shape."##
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
    /// Named color palette: key → hex (e.g. "body" → "#cc5544")
    #[serde(default)]
    palette: HashMap<String, String>,
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
    /// Normalize plan into a list of frame operations, palette, and fps.
    /// Format 1 (operations) → 1 frame. Format 2 (frames) → multiple frames.
    /// Applies palette resolution and compact format expansion to all operations.
    fn into_frames(self) -> (Vec<Vec<Value>>, HashMap<String, String>, u32) {
        let palette = self.palette;
        if !self.frames.is_empty() {
            let frames: Vec<Vec<Value>> = self
                .frames
                .into_iter()
                .take(MAX_FRAMES)
                .map(|f| preprocess_ops(f.operations, &palette))
                .collect();
            (frames, palette, self.fps.clamp(1, 30))
        } else {
            let ops = preprocess_ops(self.operations, &palette);
            (vec![ops], palette, 0) // fps=0 means single frame
        }
    }
}

// ── Operation preprocessing (palette + compact format) ──────

/// Preprocess operations: resolve palette color references and expand compact set_pixels.
fn preprocess_ops(ops: Vec<Value>, palette: &HashMap<String, String>) -> Vec<Value> {
    ops.into_iter()
        .flat_map(|op| {
            // First expand compact set_pixels if applicable
            let expanded = expand_compact_set_pixels(op);
            // Then resolve palette references in all color fields
            expanded
                .into_iter()
                .map(|o| resolve_palette_colors(o, palette))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Resolve palette key references in color fields of an operation.
/// If a "color" (or "color1", "color2", "old_color", "new_color") value matches a palette key,
/// replace it with the corresponding hex value.
fn resolve_palette_colors(mut op: Value, palette: &HashMap<String, String>) -> Value {
    if palette.is_empty() {
        return op;
    }

    let color_keys = ["color", "color1", "color2", "old_color", "new_color"];
    for key in &color_keys {
        if let Some(val) = op.get(*key).and_then(|v| v.as_str()) {
            if !val.starts_with('#') {
                if let Some(hex) = resolve_palette_key(val, palette) {
                    op[*key] = json!(hex);
                }
            }
        }
    }

    // Also resolve colors inside set_pixels pixel arrays
    if let Some(pixels) = op.get_mut("pixels").and_then(|v| v.as_array_mut()) {
        for p in pixels.iter_mut() {
            if let Some(val) = p.get("color").and_then(|v| v.as_str()).map(|s| s.to_string()) {
                if !val.starts_with('#') {
                    if let Some(hex) = resolve_palette_key(&val, palette) {
                        p["color"] = json!(hex);
                    }
                }
            }
        }
    }

    op
}

/// Resolve a palette key, with fuzzy fallback for common abbreviation mismatches.
/// E.g. model writes "wing_ol" but palette has "wing_outline", or vice versa.
fn resolve_palette_key<'a>(key: &str, palette: &'a HashMap<String, String>) -> Option<&'a String> {
    // Exact match first
    if let Some(hex) = palette.get(key) {
        return Some(hex);
    }

    // Try common suffix expansions/abbreviations
    let suffix_pairs = [
        ("_ol", "_outline"),
        ("_outline", "_ol"),
        ("_dk", "_dark"),
        ("_dark", "_dk"),
        ("_lt", "_light"),
        ("_light", "_lt"),
        ("_md", "_mid"),
        ("_mid", "_md"),
    ];

    for (short, long) in &suffix_pairs {
        if key.ends_with(short) {
            let expanded = format!("{}{}", &key[..key.len() - short.len()], long);
            if let Some(hex) = palette.get(&expanded) {
                return Some(hex);
            }
        }
    }

    // Try prefix match: find the longest palette key that starts with the same prefix
    let mut best_match: Option<(&str, &String)> = None;
    for (pk, pv) in palette {
        if pk.starts_with(key) || key.starts_with(pk.as_str()) {
            match best_match {
                None => best_match = Some((pk.as_str(), pv)),
                Some((existing, _)) => {
                    // Prefer longer common prefix
                    if pk.len() > existing.len() {
                        best_match = Some((pk.as_str(), pv));
                    }
                }
            }
        }
    }

    best_match.map(|(_, v)| v)
}

/// Expand compact set_pixels format into standard format.
///
/// Compact format (single color, coordinate string):
///   {"tool":"set_pixels","color":"#ff0000","xy":"10,20;11,20;12,21"}
///
/// Expanded to standard format:
///   {"tool":"set_pixels","pixels":[{"x":10,"y":20,"color":"#ff0000"},{"x":11,"y":20,"color":"#ff0000"},{"x":12,"y":21,"color":"#ff0000"}]}
///
/// Also supports multi-color compact format:
///   {"tool":"set_pixels","groups":[{"color":"#ff0000","xy":"10,20;11,20"},{"color":"#00ff00","xy":"5,5;6,6"}]}
///
/// If already in standard format (has "pixels" array), returns as-is.
fn expand_compact_set_pixels(op: Value) -> Vec<Value> {
    let tool = op.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    if tool != "set_pixels" {
        return vec![op];
    }

    // Already in standard format
    if op.get("pixels").is_some() {
        return vec![op];
    }

    // Single-color compact format: {"color":"...", "xy":"x,y;x,y;..."}
    if let (Some(color), Some(xy_str)) = (
        op.get("color").and_then(|v| v.as_str()),
        op.get("xy").and_then(|v| v.as_str()),
    ) {
        let pixels = parse_xy_string(xy_str, color);
        if !pixels.is_empty() {
            let mut result = Map::new();
            result.insert("tool".into(), json!("set_pixels"));
            result.insert("pixels".into(), json!(pixels));
            return vec![Value::Object(result)];
        }
    }

    // Multi-color compact format: {"groups":[{"color":"...", "xy":"..."},...]}"
    if let Some(groups) = op.get("groups").and_then(|v| v.as_array()) {
        let mut all_pixels = Vec::new();
        for group in groups {
            if let (Some(color), Some(xy_str)) = (
                group.get("color").and_then(|v| v.as_str()),
                group.get("xy").and_then(|v| v.as_str()),
            ) {
                all_pixels.extend(parse_xy_string(xy_str, color));
            }
        }
        if !all_pixels.is_empty() {
            let mut result = Map::new();
            result.insert("tool".into(), json!("set_pixels"));
            result.insert("pixels".into(), json!(all_pixels));
            return vec![Value::Object(result)];
        }
    }

    // Fallback: return as-is
    vec![op]
}

/// Parse "x,y;x,y;..." coordinate string into pixel objects with the given color.
fn parse_xy_string(xy_str: &str, color: &str) -> Vec<Value> {
    xy_str
        .split(';')
        .filter_map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return None;
            }
            let mut parts = pair.split(',');
            let x: i64 = parts.next()?.trim().parse().ok()?;
            let y: i64 = parts.next()?.trim().parse().ok()?;
            Some(json!({"x": x, "y": y, "color": color}))
        })
        .collect()
}

// ── Agent entry point ────────────────────────────────────────

/// Run the agent with multi-turn iterative drawing for single frames,
/// or single-shot for animations.
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

    // Detect if user is requesting animation
    let is_animation_request = detect_animation_request(&user_text);

    if is_animation_request {
        // ── Animation: multi-turn base frame + delta frames ──
        run_animation_multiturn(
            &user_text,
            &model,
            &editor,
            &conversation,
            offset_x,
            offset_y,
            &set_messages,
            &set_token_usage,
            &set_render_trigger,
            &stop_flag,
        )
        .await;
    } else {
        // ── Multi-turn iterative drawing (single frame) ─────
        run_multiturn_drawing(
            &user_text,
            &model,
            &editor,
            &conversation,
            offset_x,
            offset_y,
            &set_messages,
            &set_token_usage,
            &set_render_trigger,
            &stop_flag,
        )
        .await;
    }

    set_running.set(false);
}

/// Detect if the user is requesting an animation.
fn detect_animation_request(text: &str) -> bool {
    let lower = text.to_lowercase();
    let animation_keywords = [
        "animation", "animate", "アニメ", "アニメーション",
        "フレーム", "frame", "walk cycle", "walking", "running",
        "flying", "idle", "attack", "歩く", "走る", "飛ぶ",
        "歩行", "待機", "攻撃",
    ];
    animation_keywords.iter().any(|kw| lower.contains(kw))
}

/// Multi-turn iterative drawing: draw in 4 phases with canvas capture between each.
async fn run_multiturn_drawing(
    user_text: &str,
    user_model: &str,
    editor: &StoredValue<EditorState>,
    conversation: &StoredValue<Vec<ApiMessage>>,
    offset_x: i64,
    offset_y: i64,
    set_messages: &WriteSignal<Vec<ChatMessage>>,
    set_token_usage: &WriteSignal<(usize, usize)>,
    set_render_trigger: &WriteSignal<u32>,
    stop_flag: &StopFlag,
) {
    let mut total_executed = 0u32;
    // Store the palette from phase 1 — later phases reuse it
    let mut shared_palette: HashMap<String, String> = HashMap::new();

    for (phase_idx, phase) in PHASES.iter().enumerate() {
        if stop_flag.load(Ordering::Relaxed) {
            add_status(set_messages, "Stopped by user.");
            break;
        }

        let phase_num = phase_idx + 1;
        let phase_total = PHASES.len();
        add_status(
            set_messages,
            &format!(
                "Phase {}/{}: {}...",
                phase_num, phase_total, phase.name
            ),
        );

        // Build messages for this phase
        let mut messages = Vec::new();

        // First phase: user prompt only (no image)
        // Later phases: canvas image + phase instruction
        if phase.needs_image {
            // Capture current canvas
            let image_b64 = match capture_canvas_base64(editor) {
                Some(img) => img,
                None => {
                    add_status(set_messages, "Failed to capture canvas, skipping phase.");
                    continue;
                }
            };

            let palette_hint = if !shared_palette.is_empty() {
                let palette_json = serde_json::to_string(&shared_palette).unwrap_or_default();
                format!("\n\nUse this palette (from Phase 1): {palette_json}")
            } else {
                String::new()
            };

            messages.push(ApiMessage {
                role: "user".into(),
                content: vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".into(),
                            media_type: "image/png".into(),
                            data: image_b64,
                        },
                    },
                    ContentBlock::Text {
                        text: format!(
                            "Original request: \"{user_text}\"\n\nThis is the current canvas state. Apply Phase {} operations.{palette_hint}",
                            phase_num
                        ),
                    },
                ],
            });
        } else {
            // Phase 1: just the user prompt
            messages.push(ApiMessage {
                role: "user".into(),
                content: vec![ContentBlock::Text {
                    text: user_text.to_string(),
                }],
            });
        }

        // Choose model: user-selected for early phases, Sonnet for later
        let model = if phase.use_cheap_model {
            CHEAP_MODEL.to_string()
        } else {
            user_model.to_string()
        };

        let request = MessagesRequest {
            model,
            max_tokens: phase.max_tokens,
            system: phase_system_prompt(phase.name),
            messages,
            tools: vec![],
        };

        let response = match api_client::send_message(&request).await {
            Ok(r) => r,
            Err(e) => {
                add_status(set_messages, &format!("Phase {} API error: {e}", phase_num));
                continue;
            }
        };

        // Accumulate token usage
        if let Some(usage) = &response.usage {
            set_token_usage.update(|(inp, out)| {
                *inp += usage.input_tokens;
                *out += usage.output_tokens;
            });
        }

        // Warn if truncated
        if response.stop_reason.as_deref() == Some("max_tokens") {
            add_status(
                set_messages,
                &format!("Phase {}: response truncated, repairing JSON...", phase_num),
            );
        }

        // Extract and parse
        let response_text: String = response
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        let plan = match parse_plan(&response_text) {
            Ok(p) => p,
            Err(e) => {
                add_status(
                    set_messages,
                    &format!("Phase {} parse error: {e}", phase_num),
                );
                continue;
            }
        };

        // Show description
        if !plan.description.is_empty() {
            set_messages.update(|msgs| {
                msgs.push(ChatMessage::assistant(&format!(
                    "Phase {}: {}",
                    phase_num, plan.description
                )));
            });
        }

        // Capture palette from first phase, merge from subsequent phases
        if phase_idx == 0 {
            shared_palette = plan.palette.clone();
        } else {
            // Merge any new palette entries from later phases
            for (k, v) in &plan.palette {
                shared_palette.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }

        let (frames, _palette, _fps) = plan.into_frames();
        let ops = frames.into_iter().next().unwrap_or_default();

        if ops.is_empty() {
            add_status(
                set_messages,
                &format!("Phase {}: no operations (skipped).", phase_num),
            );
            continue;
        }

        // Execute operations
        let ops = &ops[..ops.len().min(MAX_OPS_PER_FRAME)];
        let executed = execute_frame_ops(ops, offset_x, offset_y, editor, set_messages, stop_flag);
        total_executed += executed;

        add_status(
            set_messages,
            &format!("Phase {}: {} ops executed.", phase_num, executed),
        );

        // Render after each phase so the canvas is updated for capture
        if executed > 0 {
            set_render_trigger.update(|v| *v += 1);
        }
    }

    // Save to conversation history (compressed summary)
    conversation.update_value(|msgs| {
        msgs.push(ApiMessage {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: user_text.to_string(),
            }],
        });
        msgs.push(ApiMessage {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: format!(
                    "[Multi-turn drawing completed: {} total ops across {} phases]",
                    total_executed,
                    PHASES.len()
                ),
            }],
        });
    });

    add_status(
        set_messages,
        &format!("Done — {} operations across {} phases.", total_executed, PHASES.len()),
    );
}

/// Animation path: multi-turn phases for frame 1, then single-shot for remaining frames.
async fn run_animation_multiturn(
    user_text: &str,
    model: &str,
    editor: &StoredValue<EditorState>,
    conversation: &StoredValue<Vec<ApiMessage>>,
    offset_x: i64,
    offset_y: i64,
    set_messages: &WriteSignal<Vec<ChatMessage>>,
    set_token_usage: &WriteSignal<(usize, usize)>,
    set_render_trigger: &WriteSignal<u32>,
    stop_flag: &StopFlag,
) {
    // ── Step 1: Draw frame 1 using multi-turn phases ─────────
    // Strip animation keywords from the prompt for phase drawing (single-frame focus)
    let frame1_prompt = format!(
        "{}\n\nDraw ONLY a single static frame (the base pose). Do NOT create animation frames. This will be used as the base for animation later.",
        user_text
    );

    add_status(set_messages, "Drawing base frame with multi-turn phases...");

    run_multiturn_drawing(
        &frame1_prompt,
        model,
        editor,
        conversation,
        offset_x,
        offset_y,
        set_messages,
        set_token_usage,
        set_render_trigger,
        stop_flag,
    )
    .await;

    if stop_flag.load(Ordering::Relaxed) {
        return;
    }

    // Save frame 1 to timeline
    editor.update_value(|state| {
        state.save_frame();
    });

    // ── Step 2: Capture frame 1 and generate animation frames ──
    add_status(set_messages, "Generating animation frames from base...");

    let image_b64 = match capture_canvas_base64(editor) {
        Some(img) => img,
        None => {
            add_status(set_messages, "Failed to capture base frame.");
            return;
        }
    };

    let anim_messages = vec![ApiMessage {
        role: "user".into(),
        content: vec![
            ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/png".into(),
                    data: image_b64,
                },
            },
            ContentBlock::Text {
                text: format!(
                    "This is frame 1 (base pose) of: \"{user_text}\"\n\n\
                    Generate the REMAINING animation frames. Frame 1 is already drawn.\n\
                    Output JSON: {{\"description\":\"...\",\"fps\":8,\"palette\":{{}},\"frames\":[{{\"operations\":[...]}},...]}}\n\n\
                    Each frame MUST start with copy_prev_frame, then apply ONLY the changes.\n\
                    Animate by moving specific body parts (wings, legs) with 2-4px shifts.\n\
                    Keep torso/body FIXED. Only limbs and accessories move.\n\
                    Use hex colors directly (not palette keys)."
                ),
            },
        ],
    }];

    let request = MessagesRequest {
        model: CHEAP_MODEL.to_string(), // Sonnet for animation delta frames
        max_tokens: 8192,
        system: animation_system_prompt(),
        messages: anim_messages,
        tools: vec![],
    };

    let response = match api_client::send_message(&request).await {
        Ok(r) => r,
        Err(e) => {
            add_status(set_messages, &format!("Animation frames API error: {e}"));
            return;
        }
    };

    if let Some(usage) = &response.usage {
        set_token_usage.update(|(inp, out)| {
            *inp += usage.input_tokens;
            *out += usage.output_tokens;
        });
    }

    if response.stop_reason.as_deref() == Some("max_tokens") {
        add_status(
            set_messages,
            "Animation frames truncated — repairing JSON...",
        );
    }

    let response_text: String = response
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    let plan = match parse_plan(&response_text) {
        Ok(p) => p,
        Err(e) => {
            add_status(set_messages, &format!("Animation parse error: {e}"));
            return;
        }
    };

    if !plan.description.is_empty() {
        set_messages.update(|msgs| {
            msgs.push(ChatMessage::assistant(&plan.description));
        });
    }

    let (frames, _palette, fps) = plan.into_frames();

    if frames.is_empty() {
        add_status(set_messages, "No animation frames generated.");
        return;
    }

    add_status(
        set_messages,
        &format!("Applying {} animation frames...", frames.len()),
    );

    let mut total_anim_ops = 0u32;

    for (frame_idx, frame_ops) in frames.iter().enumerate() {
        if stop_flag.load(Ordering::Relaxed) {
            add_status(set_messages, "Stopped by user.");
            break;
        }

        // Add new frame
        editor.update_value(|state| {
            state.add_frame();
        });

        // copy_prev_frame
        if let Some(first_op) = frame_ops.first() {
            if first_op.get("tool").and_then(|v| v.as_str()) == Some("copy_prev_frame") {
                editor.update_value(|state| {
                    let prev_idx = state.timeline.current_frame.saturating_sub(1);
                    let prev_canvas = state.timeline.frames[prev_idx].canvas.clone();
                    for (li, layer) in prev_canvas.layers.iter().enumerate() {
                        if li < state.canvas.layers.len() {
                            let fw = state.canvas.frame_width();
                            let fh = state.canvas.frame_height();
                            let fx = state.canvas.frame_x;
                            let fy = state.canvas.frame_y;
                            for y in 0..fh {
                                for x in 0..fw {
                                    let bx = fx + x;
                                    let by = fy + y;
                                    if let Some(&src) = layer.buffer.get_pixel(bx, by) {
                                        state.canvas.layers[li].buffer.set_pixel(bx, by, src);
                                    }
                                }
                            }
                        }
                    }
                });
                set_messages.update(|msgs| {
                    msgs.push(ChatMessage::tool(
                        "copy_prev_frame",
                        crate::ai::ToolStatus::Done,
                    ));
                });
            }
        }

        add_status(
            set_messages,
            &format!("Drawing frame {}/{}...", frame_idx + 2, frames.len() + 1),
        );

        let ops = &frame_ops[..frame_ops.len().min(MAX_OPS_PER_FRAME)];
        let executed = execute_frame_ops(ops, offset_x, offset_y, editor, set_messages, stop_flag);
        total_anim_ops += executed;

        editor.update_value(|state| {
            state.save_frame();
        });
    }

    // Set FPS
    if fps > 0 {
        editor.update_value(|state| {
            state.timeline.fps = fps;
            state.switch_frame(0);
        });
    }

    if total_anim_ops > 0 {
        set_render_trigger.update(|v| *v += 1);
    }

    // Save to conversation history
    conversation.update_value(|msgs| {
        msgs.push(ApiMessage {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: format!(
                    "[Animation: base frame via multi-turn + {} delta frames]",
                    frames.len()
                ),
            }],
        });
    });

    add_status(
        set_messages,
        &format!(
            "Done — {} frames, {} animation ops.",
            frames.len() + 1,
            total_anim_ops
        ),
    );
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
            Some(n) if n != "finish" && n != "clear_canvas" && n != "copy_prev_frame" => n,
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

// ── Canvas capture ────────────────────────────────────────────

/// Capture the current canvas as a base64-encoded PNG for inter-phase feedback.
fn capture_canvas_base64(editor: &StoredValue<EditorState>) -> Option<String> {
    editor.with_value(|state| {
        pxlot_formats::png_format::export_png(&state.canvas)
            .ok()
            .map(|bytes| base64::engine::general_purpose::STANDARD.encode(&bytes))
    })
}

// ── Conversation history compression ─────────────────────────

/// Compress older assistant responses in the conversation history.
/// Keeps the most recent assistant response in full (for follow-up accuracy).
/// Replaces all older assistant responses with a short text summary.
#[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Circle center (draw_filled_circle)
    if let Some(cx) = op.get("cx").and_then(|v| v.as_i64()) {
        out["cx"] = json!(clamp_and_offset(cx, dx));
    }
    if let Some(cy) = op.get("cy").and_then(|v| v.as_i64()) {
        out["cy"] = json!(clamp_and_offset(cy, dy));
    }

    // draw_filled_polygon — clamp and offset each vertex
    if let Some(vertices) = op.get("vertices").and_then(|v| v.as_array()) {
        let offset_verts: Vec<Value> = vertices
            .iter()
            .map(|v| {
                let mut vv = v.clone();
                if let Some(x) = v.get("x").and_then(|v| v.as_i64()) {
                    vv["x"] = json!(clamp_and_offset(x, dx));
                }
                if let Some(y) = v.get("y").and_then(|v| v.as_i64()) {
                    vv["y"] = json!(clamp_and_offset(y, dy));
                }
                vv
            })
            .collect();
        out["vertices"] = json!(offset_verts);
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
/// Attempts to repair common JSON issues from truncated or malformed model output.
fn parse_plan(text: &str) -> Result<DrawingPlan, String> {
    let json_str = extract_json(text)?;

    // Try direct parse first
    if let Ok(plan) = serde_json::from_str::<DrawingPlan>(json_str) {
        return Ok(plan);
    }

    // Attempt JSON repair for common LLM output issues
    let repaired = repair_json(json_str);
    serde_json::from_str::<DrawingPlan>(&repaired)
        .map_err(|e| format!("{e} (also tried JSON repair)"))
}

/// Attempt to repair common JSON issues from LLM output:
/// - Trailing commas before ] or }
/// - Unclosed brackets/braces (truncated output)
/// - Incomplete trailing elements
fn repair_json(input: &str) -> String {
    let mut s = input.trim().to_string();

    // Remove trailing commas before ] or }
    loop {
        let before = s.clone();
        s = s.replace(",]", "]").replace(",}", "}");
        // Also handle commas with whitespace before closing brackets
        while let Some(pos) = find_trailing_comma(&s) {
            s.remove(pos);
        }
        if s == before {
            break;
        }
    }

    // Count unmatched brackets and braces
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in s.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_depth += 1,
            '}' if !in_string => brace_depth -= 1,
            '[' if !in_string => bracket_depth += 1,
            ']' if !in_string => bracket_depth -= 1,
            _ => {}
        }
    }

    // If we're inside a string at the end (truncated string value), close it
    if in_string {
        s.push('"');
    }

    // Remove any incomplete trailing element after the last comma
    // e.g. `[{"tool":"x"},{"tool":"y","col` → `[{"tool":"x"}`
    if brace_depth > 0 || bracket_depth > 0 {
        // Try to find the last complete element by finding the last valid }, or ],
        if let Some(last_complete) = find_last_complete_element(&s) {
            s.truncate(last_complete + 1);
            // Recount after truncation
            brace_depth = 0;
            bracket_depth = 0;
            in_string = false;
            escape_next = false;
            for ch in s.chars() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                match ch {
                    '\\' if in_string => escape_next = true,
                    '"' => in_string = !in_string,
                    '{' if !in_string => brace_depth += 1,
                    '}' if !in_string => brace_depth -= 1,
                    '[' if !in_string => bracket_depth += 1,
                    ']' if !in_string => bracket_depth -= 1,
                    _ => {}
                }
            }
        }
    }

    // Close unclosed brackets and braces
    for _ in 0..bracket_depth {
        s.push(']');
    }
    for _ in 0..brace_depth {
        s.push('}');
    }

    // Final trailing comma cleanup after repair
    s = s.replace(",]", "]").replace(",}", "}");

    s
}

/// Find a trailing comma followed by optional whitespace and a closing bracket/brace.
fn find_trailing_comma(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b',' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\n' || bytes[j] == b'\r' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b']' || bytes[j] == b'}') {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Find the byte position of the end of the last complete JSON element.
/// Looks for the last `}` or `]` that is followed by a `,` or is at a nesting
/// boundary, indicating a complete element before a truncated one.
fn find_last_complete_element(s: &str) -> Option<usize> {
    let mut best = None;
    let mut in_string = false;
    let mut escape_next = false;
    let mut depth = 0i32;

    for (i, ch) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => depth += 1,
            '}' | ']' if !in_string => {
                depth -= 1;
                // Track positions where a complete element ends (depth >= 1 means
                // we're still inside the outer object/array)
                if depth >= 1 {
                    best = Some(i);
                }
            }
            _ => {}
        }
    }
    best
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
