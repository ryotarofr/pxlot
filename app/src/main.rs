use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod ai;
mod components;
mod i18n;
mod project_store;
mod state;
mod storage;

use components::ai_chat::AiChat;
use components::ai_panel::{AiPanel, AiResult, AiStatus};
use components::canvas_view::CanvasView;
use components::color_picker::ColorPicker;
use components::layer_panel::{LayerInfo, LayerPanel};
use components::timeline::TimelinePanel;
use components::tool_panel::ToolPanel;
use pxlot_core::image_processing::{self, DitherMethod, DownsampleMethod}; // DownsampleMethod used internally
use pxlot_core::Color;
use pxlot_formats::{gif_format, png_format};
use pxlot_tools::{apply_redo, apply_undo, ToolKind};
use i18n::{t, Lang};
use state::EditorState;

fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);
    log::info!("pxlot starting...");
    mount_to_body(App);
}

/// Trigger a browser download of binary data as a file.
fn download_bytes(data: &[u8], filename: &str, mime: &str) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let uint8arr = js_sys::Uint8Array::new_with_length(data.len() as u32);
    uint8arr.copy_from(data);

    let array = js_sys::Array::new();
    array.push(&uint8arr.buffer());

    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&array, &opts).unwrap();

    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();

    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .unwrap()
        .dyn_into()
        .unwrap();
    a.set_href(&url);
    a.set_download(filename);
    a.click();

    let _ = web_sys::Url::revoke_object_url(&url);
}

#[component]
fn App() -> impl IntoView {
    // Try to load autosave, otherwise create new canvas
    let (canvas_width, canvas_height, editor) = if let Some(saved) = storage::load_autosave() {
        let w = saved.frame_width();
        let h = saved.frame_height();
        let mut state = EditorState::new(w, h);
        state.canvas = saved.clone();
        state.timeline = pxlot_animation::Timeline::new(saved);
        // Restore undo/redo history if available
        if let Some(history) = storage::load_history() {
            state.history = history;
        }
        (w, h, StoredValue::new(state))
    } else {
        let w = 32u32;
        let h = 32u32;
        (w, h, StoredValue::new(EditorState::new(w, h)))
    };

    // i18n
    let (lang, set_lang) = i18n::provide_i18n();

    // Reactive signals for UI
    let (current_tool, set_current_tool) = signal(ToolKind::Pencil);
    let (current_color, set_current_color) = signal(Color::WHITE);
    let (render_trigger, set_render_trigger) = signal(0u32);
    let (show_grid, set_show_grid) = signal(true);

    // Layer info signal
    let (layers_info, set_layers_info) = signal(vec![LayerInfo {
        index: 0,
        name: "Layer 1".to_string(),
        visible: true,
        locked: false,
        opacity: 255,
    }]);
    let (active_layer, set_active_layer) = signal(0usize);

    // Timeline signals
    let (frame_count, set_frame_count) = signal(1usize);
    let (current_frame, set_current_frame) = signal(0usize);
    let (fps, set_fps_signal) = signal(10u32);
    let (playing, set_playing) = signal(false);

    // Color history signal
    let (color_history, set_color_history) = signal(Vec::<Color>::new());

    // AI panel signals
    let (ai_result, set_ai_result) = signal(AiResult {
        palette_hex: vec![],
        style_comment: String::new(),
        status: AiStatus::Idle,
    });
    let (is_online, _set_is_online) = signal(true);

    // AI Chat signals
    let (chat_messages, set_chat_messages) = signal(Vec::<ai::ChatMessage>::new());
    let (ai_running, set_ai_running) = signal(false);
    let (ai_model, set_ai_model) = signal("claude-sonnet-4-6".to_string());
    let (ai_token_usage, set_ai_token_usage) = signal((0usize, 0usize));
    let (chat_open, set_chat_open) = signal(false);
    let ai_stop_flag = ai::agent::new_stop_flag();
    let ai_conversation = StoredValue::new(Vec::<ai::api_client::ApiMessage>::new());

    // AI Chat callbacks
    let stop_flag_for_send = ai_stop_flag.clone();
    let on_chat_send = Callback::new(move |text: String| {
        // Prevent double execution
        if ai_running.get() {
            return;
        }

        // Add user message to chat
        set_chat_messages.update(|msgs| {
            msgs.push(ai::ChatMessage::user(&text));
        });

        // Get API key
        let Some(api_key) = ai::load_api_key() else {
            set_chat_messages.update(|msgs| {
                msgs.push(ai::ChatMessage::status("No API key configured."));
            });
            return;
        };

        let model = ai_model.get();
        let stop = stop_flag_for_send.clone();
        stop.store(false, std::sync::atomic::Ordering::Relaxed);

        // Spawn the async agent loop
        wasm_bindgen_futures::spawn_local(ai::agent::run_agent(
            text,
            api_key,
            model,
            editor,
            ai_conversation,
            set_chat_messages,
            set_ai_running,
            set_ai_token_usage,
            set_render_trigger,
            stop,
        ));
    });

    let stop_flag_for_stop = ai_stop_flag.clone();
    let on_chat_stop = Callback::new(move |_: ()| {
        stop_flag_for_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let on_chat_clear = Callback::new(move |_: ()| {
        set_chat_messages.set(Vec::new());
        set_ai_token_usage.set((0, 0));
        ai_conversation.set_value(Vec::new());
    });

    let on_chat_model_change = Callback::new(move |model: String| {
        set_ai_model.set(model);
    });

    let on_toggle_chat = Callback::new(move |_: ()| {
        set_chat_open.update(|v| *v = !*v);
    });

    let on_close_chat = Callback::new(move |_: ()| {
        set_chat_open.set(false);
    });

    // Zoom display
    let (zoom_display, set_zoom_display) = signal("16x".to_string());

    // PNG export scale
    let (png_scale, set_png_scale) = signal(8u32);

    // Status message signal (shown briefly for errors/warnings)
    let (status_message, set_status_message) = signal(Option::<String>::None);

    // Mirror/symmetry mode
    let (mirror_x, set_mirror_x) = signal(false);

    // Grid size
    let (grid_size, set_grid_size) = signal(1u32);

    // Tile preview
    let (tile_preview, set_tile_preview) = signal(false);

    // Onion skin
    let (onion_skin, set_onion_skin) = signal(false);
    let (onion_skin_frames, set_onion_skin_frames) = signal(1u32);

    // Custom export filename
    let (export_filename, set_export_filename) = signal("pxlot".to_string());

    // New canvas dialog
    let (show_new_dialog, set_show_new_dialog) = signal(false);
    let (new_canvas_width, set_new_canvas_width) = signal("32".to_string());
    let (new_canvas_height, set_new_canvas_height) = signal("32".to_string());

    // Canvas resize dialog
    let (show_resize_dialog, set_show_resize_dialog) = signal(false);
    let (resize_width, set_resize_width) = signal("32".to_string());
    let (resize_height, set_resize_height) = signal("32".to_string());

    // Canvas size signal for status bar
    let (canvas_w_signal, set_canvas_w) = signal(canvas_width);
    let (canvas_h_signal, set_canvas_h) = signal(canvas_height);

    // Helper to sync signals from editor state
    let sync_state = move || {
        editor.with_value(|state| {
            set_layers_info.set(
                state
                    .canvas
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(i, l)| LayerInfo {
                        index: i,
                        name: l.name.clone(),
                        visible: l.visible,
                        locked: l.locked,
                        opacity: l.opacity,
                    })
                    .collect(),
            );
            set_active_layer.set(state.canvas.active_layer);
            set_zoom_display.set(format!("{:.0}x", state.zoom));
            set_frame_count.set(state.timeline.frame_count());
            set_current_frame.set(state.timeline.current_frame);
            set_fps_signal.set(state.timeline.fps);
            set_playing.set(state.timeline.playing);
            set_color_history.set(state.color_history.clone());
            set_canvas_w.set(state.canvas.frame_width());
            set_canvas_h.set(state.canvas.frame_height());
        });
        // Drain status message (needs mutable access)
        editor.update_value(|state| {
            if let Some(msg) = state.status_message.take() {
                set_status_message.set(Some(msg));
            }
        });
    };

    let trigger_render = move || {
        set_render_trigger.update(|v| *v += 1);
        sync_state();
        // Auto-save
        editor.with_value(|state| {
            storage::autosave(&state.canvas, &state.history);
        });
    };

    // Sync UI state whenever render_trigger changes (covers agent-driven updates)
    Effect::new(move |_| {
        let _ = render_trigger.get(); // subscribe to changes
        sync_state();
    });

    // Sync tool/color changes to editor state
    Effect::new(move |_| {
        let tool = current_tool.get();
        editor.update_value(|state| state.current_tool = tool);
    });

    Effect::new(move |_| {
        let color = current_color.get();
        editor.update_value(|state| {
            state.current_color = color;
            state.record_color(color);
        });
    });

    Effect::new(move |_| {
        let grid = show_grid.get();
        editor.update_value(|state| state.show_grid = grid);
        set_render_trigger.update(|v| *v += 1);
    });

    Effect::new(move |_| {
        let m = mirror_x.get();
        editor.update_value(|state| state.mirror_x = m);
        set_render_trigger.update(|v| *v += 1);
    });

    Effect::new(move |_| {
        let gs = grid_size.get();
        editor.update_value(|state| state.grid_size = gs);
        set_render_trigger.update(|v| *v += 1);
    });

    Effect::new(move |_| {
        let os = onion_skin.get();
        editor.update_value(|state| state.onion_skin = os);
        set_render_trigger.update(|v| *v += 1);
    });

    Effect::new(move |_| {
        let osf = onion_skin_frames.get();
        editor.update_value(|state| state.onion_skin_frames = osf);
        set_render_trigger.update(|v| *v += 1);
    });

    // Undo handler
    let on_undo = move || {
        editor.update_value(|state| {
            if let Some(cmd) = state.history.undo() {
                let cmd = cmd.clone();
                apply_undo(&mut state.canvas, &cmd);
            }
        });
        trigger_render();
    };

    // Redo handler
    let on_redo = move || {
        editor.update_value(|state| {
            if let Some(cmd) = state.history.redo() {
                let cmd = cmd.clone();
                apply_redo(&mut state.canvas, &cmd);
            }
        });
        trigger_render();
    };

    // PNG export handler (with scale)
    let do_export_png = move || {
        let scale = png_scale.get();
        let filename = format!("{}.png", export_filename.get());
        editor.with_value(|state| {
            match png_format::export_png_scaled(&state.canvas, scale) {
                Ok(data) => {
                    download_bytes(&data, &filename, "image/png");
                }
                Err(e) => {
                    log::error!("PNG export failed: {}", e);
                    set_status_message.set(Some(format!("PNG export failed: {}", e)));
                }
            }
        });
    };

    // GIF export handler
    let do_export_gif = move || {
        let filename = format!("{}.gif", export_filename.get());
        editor.update_value(|state| {
            state.save_frame(); // ensure current frame is saved
        });
        editor.with_value(|state| {
            match gif_format::export_gif(&state.timeline) {
                Ok(data) => {
                    download_bytes(&data, &filename, "image/gif");
                }
                Err(e) => {
                    log::error!("GIF export failed: {}", e);
                    set_status_message.set(Some(format!("GIF export failed: {}", e)));
                }
            }
        });
    };

    // Project save handler
    let do_save_project = move || {
        editor.update_value(|state| {
            state.save_frame();
        });
        editor.with_value(|state| {
            let now = js_sys::Date::now();
            let data = project_store::ProjectData {
                meta: project_store::ProjectMeta {
                    name: "default".to_string(),
                    width: state.canvas.frame_width(),
                    height: state.canvas.frame_height(),
                    frame_count: state.timeline.frame_count(),
                    created_at: now,
                    updated_at: now,
                },
                timeline: state.timeline.clone(),
            };
            let Some(json) = project_store::serialize(&data) else { return };
            match project_store::check_size(json.len()) {
                project_store::SizeWarning::Exceeded(size) => {
                    log::error!(
                        "Project too large to save: {} MB",
                        size / (1024 * 1024)
                    );
                }
                project_store::SizeWarning::Warn(size) => {
                    log::warn!(
                        "Project is large: {} MB. Consider reducing layers/frames.",
                        size / (1024 * 1024)
                    );
                    project_store::save_project(data.meta.name.clone(), json);
                }
                project_store::SizeWarning::Ok => {
                    project_store::save_project(data.meta.name.clone(), json);
                }
            }
        });
    };

    // Project load handler
    let do_load_project = move || {
        project_store::load_project("default".to_string(), move |project| {
            if let Some(data) = project {
                let canvas = data.timeline.current_canvas().clone();
                editor.update_value(|state| {
                    state.canvas = canvas;
                    state.timeline = data.timeline;
                    state.history = pxlot_core::history::History::new();
                });
                trigger_render();
                log::info!("Project loaded from IndexedDB");
            } else {
                log::info!("No saved project found");
            }
        });
    };

    // PNG import handler
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    let on_import_click = move |_| {
        if let Some(input) = file_input_ref.get() {
            let html_input: &web_sys::HtmlInputElement = input.as_ref();
            html_input.click();
        }
    };

    let on_file_selected = move |_ev: web_sys::Event| {
        let Some(input) = file_input_ref.get() else { return };
        let html_input: &web_sys::HtmlInputElement = input.as_ref();
        let Some(files) = html_input.files() else { return };
        let Some(file) = files.get(0) else { return };

        let reader = web_sys::FileReader::new().unwrap();
        let reader_clone = reader.clone();

        let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let result = reader_clone.result().unwrap();
            let array_buffer = result.dyn_into::<js_sys::ArrayBuffer>().unwrap();
            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            let mut data = vec![0u8; uint8_array.length() as usize];
            uint8_array.copy_to(&mut data);

            match png_format::import_png(&data) {
                Ok(canvas) => {
                    log::info!(
                        "PNG imported as {}x{} (max {}px)",
                        canvas.width,
                        canvas.height,
                        png_format::MAX_IMPORT_DIMENSION
                    );
                    editor.update_value(|state| {
                        state.timeline = pxlot_animation::Timeline::new(canvas.clone());
                        state.canvas = canvas;
                        state.history = pxlot_core::history::History::new();
                        state.needs_center = true;
                    });
                    trigger_render();
                }
                Err(e) => {
                    log::error!("PNG import failed: {}", e);
                }
            }
        }) as Box<dyn FnMut(_)>);

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        let _ = reader.read_as_array_buffer(&file);
        onload.forget();

        html_input.set_value("");
    };

    // Layer callbacks
    let on_layer_select = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            if idx < state.canvas.layers.len() {
                state.canvas.active_layer = idx;
            }
        });
        trigger_render();
    });

    let on_layer_add = Callback::new(move |_: ()| {
        editor.update_value(|state| {
            let count = state.canvas.layers.len() + 1;
            state.canvas.add_layer(format!("Layer {}", count));
        });
        trigger_render();
    });

    let on_layer_remove = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            state.canvas.remove_layer(idx);
        });
        trigger_render();
    });

    let on_layer_toggle_vis = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            if let Some(layer) = state.canvas.layers.get_mut(idx) {
                layer.visible = !layer.visible;
            }
        });
        trigger_render();
    });

    let on_layer_opacity = Callback::new(move |(idx, opacity): (usize, u8)| {
        editor.update_value(|state| {
            if let Some(layer) = state.canvas.layers.get_mut(idx) {
                layer.opacity = opacity;
            }
        });
        trigger_render();
    });

    let on_layer_move_up = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            if idx + 1 < state.canvas.layers.len() {
                state.canvas.move_layer(idx, idx + 1);
            }
        });
        trigger_render();
    });

    let on_layer_move_down = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            if idx > 0 {
                state.canvas.move_layer(idx, idx - 1);
            }
        });
        trigger_render();
    });

    // Sprite sheet export handler
    let do_export_spritesheet = move || {
        let filename = format!("{}_sheet.png", export_filename.get());
        editor.update_value(|state| {
            state.save_frame();
        });
        editor.with_value(|state| {
            let frame_count = state.timeline.frame_count();
            if frame_count == 0 { return; }
            let fw = state.canvas.frame_width();
            let fh = state.canvas.frame_height();
            // Lay out frames horizontally
            let sheet_w = fw * frame_count as u32;
            let sheet_h = fh;
            let mut sheet_canvas = pxlot_core::Canvas::new(sheet_w, sheet_h);
            let sfx = sheet_canvas.frame_x;
            let sfy = sheet_canvas.frame_y;
            for (i, frame) in state.timeline.frames.iter().enumerate() {
                let flat = frame.canvas.flatten_frame();
                let ox = i as u32 * fw;
                for y in 0..fh {
                    for x in 0..fw {
                        let color = *flat.get_pixel(x, y).unwrap_or(&Color::TRANSPARENT);
                        if color.a > 0 {
                            sheet_canvas.layers[0].buffer.set_pixel(sfx + ox + x, sfy + y, color);
                        }
                    }
                }
            }
            match png_format::export_png_scaled(&sheet_canvas, 1) {
                Ok(data) => download_bytes(&data, &filename, "image/png"),
                Err(e) => {
                    log::error!("Sprite sheet export failed: {}", e);
                    set_status_message.set(Some(format!("Sprite sheet export failed: {}", e)));
                }
            }
        });
    };

    // Timeline callbacks
    let on_select_frame = Callback::new(move |idx: usize| {
        editor.update_value(|state| {
            state.switch_frame(idx);
        });
        trigger_render();
    });

    let on_add_frame = Callback::new(move |_: ()| {
        editor.update_value(|state| {
            state.add_frame();
        });
        trigger_render();
    });

    let on_duplicate_frame = Callback::new(move |_: ()| {
        editor.update_value(|state| {
            state.duplicate_frame();
        });
        trigger_render();
    });

    let on_remove_frame = Callback::new(move |_: ()| {
        editor.update_value(|state| {
            state.remove_frame();
        });
        trigger_render();
    });

    let on_prev_frame = Callback::new(move |_: ()| {
        editor.update_value(|state| state.prev_frame());
        trigger_render();
    });

    let on_next_frame = Callback::new(move |_: ()| {
        editor.update_value(|state| state.next_frame());
        trigger_render();
    });

    let on_toggle_play = Callback::new(move |_: ()| {
        editor.update_value(|state| {
            state.timeline.toggle_play();
        });
        // If starting playback, kick off the animation loop
        let is_playing = editor.with_value(|s| s.timeline.playing);
        if is_playing {
            // Start playback interval using JS
            let cb = Closure::wrap(Box::new(move || {
                let still_playing = editor.with_value(|s| s.timeline.playing);
                if still_playing {
                    editor.update_value(|state| state.next_frame());
                    trigger_render();
                }
            }) as Box<dyn FnMut()>);

            let delay = editor.with_value(|s| s.timeline.frame_delay_ms());
            let window = web_sys::window().unwrap();
            let interval_id = window
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    delay as i32,
                )
                .unwrap();
            cb.forget();

            // Store interval ID to clear later
            editor.update_value(|state| {
                state.playback_interval = Some(interval_id);
            });
        } else {
            // Stop playback
            editor.update_value(|state| {
                if let Some(id) = state.playback_interval.take() {
                    let window = web_sys::window().unwrap();
                    window.clear_interval_with_handle(id);
                }
            });
        }
        sync_state();
    });

    let on_fps_change = Callback::new(move |new_fps: u32| {
        editor.update_value(|state| {
            state.timeline.fps = new_fps;
        });
        sync_state();
    });

    let on_toggle_onion_skin = Callback::new(move |_: ()| {
        set_onion_skin.update(|v| *v = !*v);
    });

    let on_onion_skin_frames_change = Callback::new(move |n: u32| {
        set_onion_skin_frames.set(n);
    });

    // AI panel callbacks
    let on_pixelize = Callback::new(move |(w, h, colors, dither): (u32, u32, usize, DitherMethod)| {
        set_ai_result.set(AiResult {
            palette_hex: vec![],
            style_comment: String::new(),
            status: AiStatus::Loading,
        });
        editor.update_value(|state| {
            let params = image_processing::PixelizeParams {
                target_width: w,
                target_height: h,
                max_colors: colors,
                dither,
                downsample: DownsampleMethod::NearestNeighbor,
                palette: None,
            };
            let flat = state.canvas.flatten_frame();
            let (result, _palette) = image_processing::pixelize(&flat, &params);
            let new_canvas = image_processing::buffer_to_canvas(result);
            state.timeline = pxlot_animation::Timeline::new(new_canvas.clone());
            state.canvas = new_canvas;
            state.history = pxlot_core::history::History::new();
        });
        set_ai_result.set(AiResult {
            palette_hex: vec![],
            style_comment: "Pixelization complete.".to_string(),
            status: AiStatus::Success,
        });
        trigger_render();
    });

    let on_extract_palette = Callback::new(move |colors: usize| {
        set_ai_result.set(AiResult {
            palette_hex: vec![],
            style_comment: String::new(),
            status: AiStatus::Loading,
        });
        let palette = editor.with_value(|state| {
            let flat = state.canvas.flatten_frame();
            image_processing::extract_palette(&flat, colors)
        });
        let hex_colors: Vec<String> = palette
            .iter()
            .map(|c| c.to_hex())
            .collect();
        set_ai_result.set(AiResult {
            palette_hex: hex_colors,
            style_comment: format!("Extracted {} colors.", palette.len()),
            status: AiStatus::Success,
        });
    });

    let on_apply_palette = Callback::new(move |hex_colors: Vec<String>| {
        editor.update_value(|state| {
            let palette: Vec<Color> = hex_colors
                .iter()
                .filter_map(|h| Color::from_hex(h))
                .collect();
            if palette.is_empty() {
                return;
            }
            image_processing::reduce_colors(
                &mut state.canvas.layers[state.canvas.active_layer].buffer,
                &palette,
                DitherMethod::None,
            );
        });
        trigger_render();
    });

    // Keyboard shortcuts
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        let ctrl = ev.ctrl_key() || ev.meta_key();
        let shift = ev.shift_key();

        match key.as_str() {
            "z" | "Z" if ctrl && shift => {
                ev.prevent_default();
                on_redo();
            }
            "z" | "Z" if ctrl => {
                ev.prevent_default();
                on_undo();
            }
            "s" | "S" if ctrl => {
                ev.prevent_default();
                do_export_png();
            }
            "n" | "N" if ctrl => {
                ev.prevent_default();
                set_show_new_dialog.set(true);
            }
            "x" | "X" if !ctrl => {
                // Swap foreground/background colors
                let secondary = editor.with_value(|s| s.secondary_color);
                let primary = current_color.get();
                set_current_color.set(secondary);
                editor.update_value(|state| state.secondary_color = primary);
            }
            "b" | "B" if !ctrl => set_current_tool.set(ToolKind::Pencil),
            "e" | "E" if !ctrl => set_current_tool.set(ToolKind::Eraser),
            "g" | "G" if !ctrl => set_current_tool.set(ToolKind::Fill),
            "i" | "I" if !ctrl => set_current_tool.set(ToolKind::Eyedropper),
            "l" | "L" if !ctrl => set_current_tool.set(ToolKind::Line),
            "R" if !ctrl && shift => set_current_tool.set(ToolKind::FilledRectangle),
            "r" if !ctrl => set_current_tool.set(ToolKind::Rectangle),
            "O" if !ctrl && shift => set_current_tool.set(ToolKind::FilledEllipse),
            "o" if !ctrl => set_current_tool.set(ToolKind::Ellipse),
            "m" | "M" if !ctrl => set_current_tool.set(ToolKind::RectSelect),
            "ArrowLeft" if !ctrl => on_prev_frame.run(()),
            "ArrowRight" if !ctrl => on_next_frame.run(()),
            " " => {
                ev.prevent_default();
                // Space held = hand tool for panning
                editor.update_value(|state| {
                    state.space_held = true;
                });
            }
            "c" | "C" if ctrl => {
                ev.prevent_default();
                // Copy selection
                editor.update_value(|state| {
                    if let Some((sx, sy, sw, sh)) = state.selection {
                        let mut pixels = Vec::new();
                        if let Some(layer) = state.canvas.active_layer_ref() {
                            for y in sy..(sy + sh) {
                                for x in sx..(sx + sw) {
                                    if x >= 0 && y >= 0 && (x as u32) < state.canvas.width && (y as u32) < state.canvas.height {
                                        pixels.push(*layer.buffer.get_pixel(x as u32, y as u32).unwrap_or(&Color::TRANSPARENT));
                                    } else {
                                        pixels.push(Color::TRANSPARENT);
                                    }
                                }
                            }
                            state.clipboard = Some(crate::state::ClipboardData {
                                width: sw as u32,
                                height: sh as u32,
                                pixels,
                            });
                        }
                    }
                });
            }
            "v" | "V" if ctrl => {
                ev.prevent_default();
                // Paste at selection origin or (0,0)
                editor.update_value(|state| {
                    let clip = state.clipboard.clone();
                    if let Some(clip) = clip {
                        let default_paste = (state.canvas.frame_x as i32, state.canvas.frame_y as i32);
                        let (ox, oy) = state.selection.map(|(x, y, _, _)| (x, y)).unwrap_or(default_paste);
                        let mut cmd = pxlot_core::history::Command::new("Paste");
                        for cy in 0..clip.height {
                            for cx in 0..clip.width {
                                let px = ox + cx as i32;
                                let py = oy + cy as i32;
                                if px >= 0 && py >= 0 && (px as u32) < state.canvas.width && (py as u32) < state.canvas.height {
                                    let color = clip.pixels[(cy * clip.width + cx) as usize];
                                    pxlot_tools::pencil_pixel(&mut state.canvas, px as u32, py as u32, color, &mut cmd);
                                }
                            }
                        }
                        state.history.push(cmd);
                    }
                });
                trigger_render();
            }
            "x" | "X" if ctrl => {
                ev.prevent_default();
                // Cut selection (copy + clear)
                editor.update_value(|state| {
                    if let Some((sx, sy, sw, sh)) = state.selection {
                        let mut pixels = Vec::new();
                        let mut cmd = pxlot_core::history::Command::new("Cut");
                        if let Some(_) = state.canvas.active_layer_ref() {
                            for y in sy..(sy + sh) {
                                for x in sx..(sx + sw) {
                                    if x >= 0 && y >= 0 && (x as u32) < state.canvas.width && (y as u32) < state.canvas.height {
                                        let layer = &state.canvas.layers[state.canvas.active_layer];
                                        pixels.push(*layer.buffer.get_pixel(x as u32, y as u32).unwrap_or(&Color::TRANSPARENT));
                                        pxlot_tools::pencil_pixel(&mut state.canvas, x as u32, y as u32, Color::TRANSPARENT, &mut cmd);
                                    } else {
                                        pixels.push(Color::TRANSPARENT);
                                    }
                                }
                            }
                            state.clipboard = Some(crate::state::ClipboardData {
                                width: sw as u32,
                                height: sh as u32,
                                pixels,
                            });
                            state.history.push(cmd);
                        }
                    }
                });
                trigger_render();
            }
            "p" | "P" if !ctrl => {
                on_toggle_play.run(());
            }
            "Home" if !ctrl => {
                // Center view on frame
                editor.update_value(|state| {
                    state.needs_center = true;
                });
                trigger_render();
            }
            _ => {}
        }
    };

    let on_keyup = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == " " {
            editor.update_value(|state| {
                state.space_held = false;
            });
        }
    };

    // Memory usage display
    let memory_display = move || {
        editor.with_value(|state| {
            let mb = state.canvas.memory_usage() as f64 / (1024.0 * 1024.0);
            format!("Mem: {:.1}MB", mb)
        })
    };

    view! {
        <div class="app" tabindex="0" on:keydown=on_keydown on:keyup=on_keyup>
            <header class="menu-bar">
                <span class="app-title">{move || t(lang.get(), "app_title")}</span>
                <div class="menu-actions">
                    <button class="menu-btn" on:click=move |_| set_show_new_dialog.set(true) title="New Canvas (Ctrl+N)">
                        "New"
                    </button>
                    <button class="menu-btn" on:click=move |_| {
                        let (w, h) = editor.with_value(|s| (s.canvas.frame_width(), s.canvas.frame_height()));
                        set_resize_width.set(w.to_string());
                        set_resize_height.set(h.to_string());
                        set_show_resize_dialog.set(true);
                    } title="Resize Canvas">
                        "Resize"
                    </button>
                    <input
                        type="text"
                        class="filename-input"
                        prop:value=move || export_filename.get()
                        on:input=move |ev| set_export_filename.set(event_target_value(&ev))
                        title="Export filename (without extension)"
                        placeholder="pxlot"
                        style="width: 80px;"
                    />
                    <select
                        class="png-scale-select"
                        on:change=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                set_png_scale.set(v);
                            }
                        }
                        title="PNG export scale"
                    >
                        <option value="4" selected=move || png_scale.get() == 4>"4x"</option>
                        <option value="8" selected=move || png_scale.get() == 8>"8x"</option>
                        <option value="16" selected=move || png_scale.get() == 16>"16x"</option>
                    </select>
                    <button class="menu-btn" on:click=move |_| do_export_png() title="Export PNG (Ctrl+S)">
                        {move || t(lang.get(), "export_png")}
                    </button>
                    <button class="menu-btn" on:click=move |_| do_export_gif() title="Export GIF">
                        {move || t(lang.get(), "export_gif")}
                    </button>
                    <button class="menu-btn" on:click=move |_| do_export_spritesheet() title="Export Sprite Sheet">
                        "Sheet"
                    </button>
                    <button class="menu-btn" on:click=on_import_click title="Import PNG">
                        {move || t(lang.get(), "import")}
                    </button>
                    <button class="menu-btn" on:click=move |_| do_save_project() title="Save Project">
                        {move || t(lang.get(), "save_project")}
                    </button>
                    <button class="menu-btn" on:click=move |_| do_load_project() title="Load Project">
                        {move || t(lang.get(), "load_project")}
                    </button>
                    <input
                        node_ref=file_input_ref
                        type="file"
                        accept=".png"
                        style="display: none"
                        on:change=on_file_selected
                    />
                    <button class="menu-btn" on:click=move |_| on_undo() title="Undo (Ctrl+Z)">
                        {move || t(lang.get(), "undo")}
                    </button>
                    <button class="menu-btn" on:click=move |_| on_redo() title="Redo (Ctrl+Shift+Z)">
                        {move || t(lang.get(), "redo")}
                    </button>
                    <label class="menu-checkbox">
                        <input
                            type="checkbox"
                            prop:checked=move || show_grid.get()
                            on:change=move |ev| {
                                set_show_grid.set(event_target_checked(&ev));
                            }
                        />
                        {move || format!(" {}", t(lang.get(), "grid"))}
                    </label>
                    <select
                        class="grid-size-select"
                        on:change=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                set_grid_size.set(v);
                            }
                        }
                        title="Grid cell size"
                    >
                        <option value="1" selected=move || grid_size.get() == 1>"1px"</option>
                        <option value="2" selected=move || grid_size.get() == 2>"2px"</option>
                        <option value="4" selected=move || grid_size.get() == 4>"4px"</option>
                        <option value="8" selected=move || grid_size.get() == 8>"8px"</option>
                        <option value="16" selected=move || grid_size.get() == 16>"16px"</option>
                    </select>
                    <label class="menu-checkbox">
                        <input
                            type="checkbox"
                            prop:checked=move || mirror_x.get()
                            on:change=move |ev| {
                                set_mirror_x.set(event_target_checked(&ev));
                            }
                        />
                        " Mirror"
                    </label>
                    <label class="menu-checkbox">
                        <input
                            type="checkbox"
                            prop:checked=move || tile_preview.get()
                            on:change=move |ev| {
                                set_tile_preview.set(event_target_checked(&ev));
                            }
                        />
                        " Tile"
                    </label>
                    <button
                        class="menu-btn lang-toggle"
                        on:click=move |_| {
                            set_lang.set(if lang.get() == Lang::En { Lang::Ja } else { Lang::En });
                        }
                    >
                        {move || lang.get().label()}
                    </button>
                </div>
            </header>
            <main class="workspace">
                <ToolPanel
                    current_tool=current_tool
                    set_tool=set_current_tool
                    chat_open=chat_open
                    on_toggle_chat=on_toggle_chat
                />
                <AiChat
                    messages=chat_messages
                    is_running=ai_running
                    is_open=chat_open
                    on_close=on_close_chat
                    on_send=on_chat_send
                    on_stop=on_chat_stop
                    on_clear=on_chat_clear
                    on_model_change=on_chat_model_change
                    model=ai_model
                    token_usage=ai_token_usage
                />
                <div class="canvas-area">
                    <CanvasView editor=editor render_trigger=render_trigger set_color=set_current_color />
                </div>
                <aside class="right-panel">
                    <LayerPanel
                        layers=layers_info
                        active_layer=active_layer
                        on_select=on_layer_select
                        on_add=on_layer_add
                        on_remove=on_layer_remove
                        on_toggle_visibility=on_layer_toggle_vis
                        on_opacity_change=on_layer_opacity
                        on_move_up=on_layer_move_up
                        on_move_down=on_layer_move_down
                    />
                    <ColorPicker
                        current_color=current_color
                        set_color=set_current_color
                        color_history=color_history
                    />
                    <AiPanel
                        on_pixelize=on_pixelize
                        on_extract_palette=on_extract_palette
                        on_apply_palette=on_apply_palette
                        ai_result=ai_result
                        is_online=is_online
                    />
                </aside>
            </main>
            <TimelinePanel
                frame_count=frame_count
                current_frame=current_frame
                fps=fps
                playing=playing
                onion_skin=onion_skin
                onion_skin_frames=onion_skin_frames
                on_select_frame=on_select_frame
                on_add_frame=on_add_frame
                on_duplicate_frame=on_duplicate_frame
                on_remove_frame=on_remove_frame
                on_prev_frame=on_prev_frame
                on_next_frame=on_next_frame
                on_toggle_play=on_toggle_play
                on_fps_change=on_fps_change
                on_toggle_onion_skin=on_toggle_onion_skin
                on_onion_skin_frames_change=on_onion_skin_frames_change
            />
            // New Canvas Dialog
            {move || {
                if show_new_dialog.get() {
                    Some(view! {
                        <div class="modal-overlay" on:click=move |_| set_show_new_dialog.set(false)>
                            <div class="modal-dialog" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                                <h3>"New Canvas"</h3>
                                <label>"Width: "
                                    <input type="number" min="1" max="256"
                                        prop:value=move || new_canvas_width.get()
                                        on:input=move |ev| set_new_canvas_width.set(event_target_value(&ev))
                                    />
                                </label>
                                <label>"Height: "
                                    <input type="number" min="1" max="256"
                                        prop:value=move || new_canvas_height.get()
                                        on:input=move |ev| set_new_canvas_height.set(event_target_value(&ev))
                                    />
                                </label>
                                <div class="modal-preset-btns">
                                    <button class="menu-btn" on:click=move |_| { set_new_canvas_width.set("16".into()); set_new_canvas_height.set("16".into()); }>"16x16"</button>
                                    <button class="menu-btn" on:click=move |_| { set_new_canvas_width.set("32".into()); set_new_canvas_height.set("32".into()); }>"32x32"</button>
                                    <button class="menu-btn" on:click=move |_| { set_new_canvas_width.set("64".into()); set_new_canvas_height.set("64".into()); }>"64x64"</button>
                                    <button class="menu-btn" on:click=move |_| { set_new_canvas_width.set("128".into()); set_new_canvas_height.set("128".into()); }>"128x128"</button>
                                </div>
                                <div class="modal-actions">
                                    <button class="menu-btn" on:click=move |_| {
                                        let w: u32 = new_canvas_width.get().parse().unwrap_or(32).clamp(1, 256);
                                        let h: u32 = new_canvas_height.get().parse().unwrap_or(32).clamp(1, 256);
                                        editor.update_value(|state| {
                                            let canvas = pxlot_core::Canvas::new(w, h);
                                            state.timeline = pxlot_animation::Timeline::new(canvas.clone());
                                            state.canvas = canvas;
                                            state.history = pxlot_core::history::History::new();
                                            state.selection = None;
                                            state.needs_center = true;
                                        });
                                        set_show_new_dialog.set(false);
                                        trigger_render();
                                    }>"Create"</button>
                                    <button class="menu-btn" on:click=move |_| set_show_new_dialog.set(false)>"Cancel"</button>
                                </div>
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}
            // Resize Canvas Dialog
            {move || {
                if show_resize_dialog.get() {
                    Some(view! {
                        <div class="modal-overlay" on:click=move |_| set_show_resize_dialog.set(false)>
                            <div class="modal-dialog" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                                <h3>"Resize Canvas"</h3>
                                <label>"Width: "
                                    <input type="number" min="1" max="256"
                                        prop:value=move || resize_width.get()
                                        on:input=move |ev| set_resize_width.set(event_target_value(&ev))
                                    />
                                </label>
                                <label>"Height: "
                                    <input type="number" min="1" max="256"
                                        prop:value=move || resize_height.get()
                                        on:input=move |ev| set_resize_height.set(event_target_value(&ev))
                                    />
                                </label>
                                <div class="modal-actions">
                                    <button class="menu-btn" on:click=move |_| {
                                        let nw: u32 = resize_width.get().parse().unwrap_or(32).clamp(1, 256);
                                        let nh: u32 = resize_height.get().parse().unwrap_or(32).clamp(1, 256);
                                        editor.update_value(|state| {
                                            let old = &state.canvas;
                                            let old_fw = old.frame_width();
                                            let old_fh = old.frame_height();
                                            let old_fx = old.frame_x;
                                            let old_fy = old.frame_y;
                                            let mut new_canvas = pxlot_core::Canvas::new(nw, nh);
                                            let new_fx = new_canvas.frame_x;
                                            let new_fy = new_canvas.frame_y;
                                            // Copy layers
                                            new_canvas.layers.clear();
                                            for layer in &old.layers {
                                                let mut new_layer = pxlot_core::Layer::new(layer.name.clone(), new_canvas.width, new_canvas.height);
                                                new_layer.visible = layer.visible;
                                                new_layer.locked = layer.locked;
                                                new_layer.opacity = layer.opacity;
                                                // Copy frame pixels that fit
                                                let copy_w = old_fw.min(nw);
                                                let copy_h = old_fh.min(nh);
                                                for y in 0..copy_h {
                                                    for x in 0..copy_w {
                                                        if let Some(&c) = layer.buffer.get_pixel(old_fx + x, old_fy + y) {
                                                            new_layer.buffer.set_pixel(new_fx + x, new_fy + y, c);
                                                        }
                                                    }
                                                }
                                                new_canvas.layers.push(new_layer);
                                            }
                                            new_canvas.active_layer = old.active_layer.min(new_canvas.layers.len().saturating_sub(1));
                                            state.canvas = new_canvas.clone();
                                            state.timeline = pxlot_animation::Timeline::new(new_canvas);
                                            state.history = pxlot_core::history::History::new();
                                            state.selection = None;
                                            state.needs_center = true;
                                        });
                                        set_show_resize_dialog.set(false);
                                        trigger_render();
                                    }>"Resize"</button>
                                    <button class="menu-btn" on:click=move |_| set_show_resize_dialog.set(false)>"Cancel"</button>
                                </div>
                            </div>
                        </div>
                    })
                } else {
                    None
                }
            }}
            <footer class="status-bar">
                {move || {
                    status_message.get().map(|msg| {
                        // Auto-clear after showing
                        let set_msg = set_status_message;
                        let cb = Closure::wrap(Box::new(move || {
                            set_msg.set(None);
                        }) as Box<dyn FnMut()>);
                        let window = web_sys::window().unwrap();
                        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                            cb.as_ref().unchecked_ref(),
                            3000,
                        );
                        cb.forget();
                        view! {
                            <span class="status-message">{msg}</span>
                        }
                    })
                }}
                <span>
                    {move || {
                        editor.with_value(|s| {
                            if s.hover_x >= 0 && s.hover_y >= 0 {
                                let rx = s.hover_x - s.canvas.frame_x as i32;
                                let ry = s.hover_y - s.canvas.frame_y as i32;
                                format!("Pos: {},{}", rx, ry)
                            } else {
                                "Pos: --".to_string()
                            }
                        })
                    }}
                </span>
                <span>{move || format!("Canvas: {}x{}", canvas_w_signal.get(), canvas_h_signal.get())}</span>
                <span>{move || format!("Zoom: {}", zoom_display.get())}</span>
                <span>{memory_display}</span>
                <span>
                    {move || {
                        editor
                            .with_value(|s| {
                                format!(
                                    "History: {}/{}",
                                    s.history.undo_count(),
                                    s.history.redo_count(),
                                )
                            })
                    }}
                </span>
                <span>
                    {move || {
                        format!(
                            "Frame: {}/{}",
                            current_frame.get() + 1,
                            frame_count.get(),
                        )
                    }}
                </span>
            </footer>
        </div>
    }
}
