use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::ai::{ChatContent, ChatMessage, ChatRole, ToolStatus};

const MIN_WIDTH: f64 = 150.0;
const MAX_WIDTH: f64 = 1000.0;
const DEFAULT_WIDTH: f64 = 300.0;

/// AI Chat panel component — left-side collapsible chat for pixel art generation.
#[component]
pub fn AiChat(
    /// Current conversation messages.
    messages: ReadSignal<Vec<ChatMessage>>,
    /// Whether the agent loop is running.
    is_running: ReadSignal<bool>,
    /// Whether the panel is open.
    is_open: ReadSignal<bool>,
    /// Close the panel.
    on_close: Callback<()>,
    /// Send a user message.
    on_send: Callback<String>,
    /// Stop the agent loop.
    on_stop: Callback<()>,
    /// Clear conversation.
    on_clear: Callback<()>,
    /// Change model.
    on_model_change: Callback<String>,
    /// Current model name.
    model: ReadSignal<String>,
    /// Token usage display.
    token_usage: ReadSignal<(usize, usize)>,
) -> impl IntoView {
    let (input_text, set_input_text) = signal(String::new());
    let (panel_width, set_panel_width) = signal(DEFAULT_WIDTH);
    let (is_resizing, set_is_resizing) = signal(false);
    let message_container_ref = NodeRef::<leptos::html::Div>::new();

    // Drag-resize handler
    let on_resize_start = move |ev: web_sys::MouseEvent| {
        ev.prevent_default();
        set_is_resizing.set(true);

        let window = web_sys::window().unwrap();

        // Create mousemove closure and extract its JS function reference
        let on_move = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MouseEvent)>::new(
            move |ev: web_sys::MouseEvent| {
                let x = ev.client_x() as f64;
                let tool_panel_width = 44.0;
                let new_width = (x - tool_panel_width).clamp(MIN_WIDTH, MAX_WIDTH);
                set_panel_width.set(new_width);
            },
        );
        let move_fn: js_sys::Function =
            on_move.as_ref().unchecked_ref::<js_sys::Function>().clone();
        // Keep alive until mouseup
        std::mem::forget(on_move);

        let move_fn_clone = move_fn.clone();
        let on_up = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MouseEvent)>::once(
            move |_: web_sys::MouseEvent| {
                set_is_resizing.set(false);
                let win = web_sys::window().unwrap();
                let _ = win.remove_event_listener_with_callback("mousemove", &move_fn_clone);
            },
        );

        let _ = window.add_event_listener_with_callback("mousemove", &move_fn);

        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_once(true);
        let _ = window.add_event_listener_with_callback_and_add_event_listener_options(
            "mouseup",
            on_up.as_ref().unchecked_ref(),
            &opts,
        );
        on_up.forget();
    };

    // Auto-scroll to bottom when messages change
    let scroll_to_bottom = move || {
        if let Some(el) = message_container_ref.get() {
            let el: &web_sys::HtmlElement = &el;
            el.set_scroll_top(el.scroll_height());
        }
    };

    // Send message handler
    let do_send = move || {
        let text = input_text.get().trim().to_string();
        if text.is_empty() || is_running.get() {
            return;
        }
        on_send.run(text);
        set_input_text.set(String::new());
    };

    let on_input_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            do_send();
        }
    };

    view! {
        <aside
            class="ai-chat-sidebar"
            class:ai-chat-sidebar-open=move || is_open.get()
            class:ai-chat-resizing=move || is_resizing.get()
            style:width=move || {
                if is_open.get() {
                    format!("{}px", panel_width.get())
                } else {
                    "0px".to_string()
                }
            }
        >
            // Header
            <div class="ai-chat-header">
                <h3>"AI Chat"</h3>
                <div class="ai-chat-header-actions">
                    <button
                        class="ai-chat-icon-btn"
                        title="Clear Chat"
                        on:click=move |_| on_clear.run(())
                    >
                        "C"
                    </button>
                    <button
                        class="ai-chat-icon-btn"
                        title="Close"
                        on:click=move |_| on_close.run(())
                    >
                        "\u{2190}"
                    </button>
                </div>
            </div>

            // Model selector
            <div class="ai-model-select">
                <select
                    class="ai-select"
                    on:change=move |ev| on_model_change.run(event_target_value(&ev))
                >
                    <option
                        value="claude-sonnet-4-6"
                        selected=move || model.get() == "claude-sonnet-4-6"
                    >
                        "Sonnet 4.6"
                    </option>
                    <option
                        value="claude-haiku-4-5-20251001"
                        selected=move || model.get() == "claude-haiku-4-5-20251001"
                    >
                        "Haiku 4.5"
                    </option>
                </select>
                <span class="ai-token-usage">
                    {move || {
                        let (inp, out) = token_usage.get();
                        if inp > 0 || out > 0 {
                            format!("{}k/{}k", inp / 1000, out / 1000)
                        } else {
                            String::new()
                        }
                    }}
                </span>
            </div>

            // Messages area
            <div class="ai-chat-messages" node_ref=message_container_ref>
                {move || {
                    let msgs = messages.get();
                    scroll_to_bottom();
                    if msgs.is_empty() {
                        vec![view! {
                            <div class="ai-chat-empty">
                                "Describe what you want to draw. The AI will use the editor tools to create pixel art."
                            </div>
                        }.into_any()]
                    } else {
                        msgs.iter().map(|msg| {
                            render_message(msg)
                        }).collect::<Vec<_>>()
                    }
                }}
            </div>

            // Input area
            <div class="ai-chat-input-area">
                <textarea
                    class="ai-chat-input"
                    placeholder=move || {
                        if is_running.get() {
                            "AI is working..."
                        } else {
                            "Describe pixel art to create..."
                        }
                    }
                    prop:value=move || input_text.get()
                    prop:disabled=move || is_running.get()
                    on:input=move |ev| set_input_text.set(event_target_value(&ev))
                    on:keydown=on_input_keydown
                    rows="3"
                />
                <div class="ai-chat-actions">
                    {move || {
                        if is_running.get() {
                            view! {
                                <button
                                    class="ai-chat-stop-btn"
                                    on:click=move |_| on_stop.run(())
                                >
                                    "Stop"
                                </button>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    class="ai-chat-send-btn"
                                    prop:disabled=move || {
                                        input_text.get().trim().is_empty()
                                    }
                                    on:click=move |_| do_send()
                                >
                                    "Send"
                                </button>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            // Resize handle on right edge
            <div
                class="ai-chat-resize-handle"
                on:mousedown=on_resize_start
            ></div>
        </aside>
    }
}

/// Render a single chat message.
fn render_message(msg: &ChatMessage) -> leptos::prelude::AnyView {
    let role_class = match msg.role {
        ChatRole::User => "ai-msg-user",
        ChatRole::Assistant => "ai-msg-assistant",
    };

    match &msg.content {
        ChatContent::Text(text) => {
            let text = text.clone();
            view! {
                <div class=format!("ai-chat-msg {}", role_class)>
                    <span class="ai-msg-role">
                        {match msg.role {
                            ChatRole::User => "> ",
                            ChatRole::Assistant => "AI: ",
                        }}
                    </span>
                    <span class="ai-msg-text">{text}</span>
                </div>
            }
            .into_any()
        }
        ChatContent::ToolUse { name, status } => {
            let status_class = match status {
                ToolStatus::Running => "ai-tool-running",
                ToolStatus::Done => "ai-tool-done",
                ToolStatus::Error(_) => "ai-tool-error",
            };
            let status_text = match status {
                ToolStatus::Running => "...".to_string(),
                ToolStatus::Done => "OK".to_string(),
                ToolStatus::Error(e) => e.clone(),
            };
            let name = name.clone();
            view! {
                <div class=format!("ai-chat-msg ai-chat-tool {}", status_class)>
                    <span class="ai-tool-name">{format!("[{}]", name)}</span>
                    <span class="ai-tool-status">{status_text}</span>
                </div>
            }
            .into_any()
        }
        ChatContent::Status(text) => {
            let text = text.clone();
            view! {
                <div class="ai-chat-msg ai-chat-status">
                    <span>{text}</span>
                </div>
            }
            .into_any()
        }
    }
}
