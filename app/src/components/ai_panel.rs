use leptos::prelude::*;
use pxlot_core::image_processing::DitherMethod;

/// AI analysis mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    Pixelate,
    Palette,
}

/// AI panel state shared with the main app.
#[derive(Clone, Debug)]
pub struct AiResult {
    pub palette_hex: Vec<String>,
    pub style_comment: String,
    pub status: AiStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiStatus {
    Idle,
    Loading,
    Success,
    Error(String),
    Offline,
}

#[component]
pub fn AiPanel(
    on_pixelize: Callback<(u32, u32, usize, DitherMethod)>,
    on_extract_palette: Callback<usize>,
    on_apply_palette: Callback<Vec<String>>,
    ai_result: ReadSignal<AiResult>,
    is_online: ReadSignal<bool>,
) -> impl IntoView {
    let (mode, set_mode) = signal(AiMode::Pixelate);
    let (target_size, set_target_size) = signal("32".to_string());
    let (max_colors, set_max_colors) = signal("16".to_string());
    let (dither, set_dither) = signal(DitherMethod::None);

    let on_execute = move |_| {
        let m = mode.get();
        match m {
            AiMode::Pixelate => {
                let size: u32 = target_size.get().parse().unwrap_or(32);
                let colors: usize = max_colors.get().parse().unwrap_or(16);
                on_pixelize.run((size, size, colors, dither.get()));
            }
            AiMode::Palette => {
                let colors: usize = max_colors.get().parse().unwrap_or(16);
                on_extract_palette.run(colors);
            }
        }
    };

    view! {
        <div class="panel-section ai-panel" role="region" aria-label="AI Assistant">
            <h3>"AI Assistant"</h3>

            {move || {
                if !is_online.get() {
                    Some(view! {
                        <div class="ai-offline-notice">
                            "Offline - AI features unavailable. Local processing still works."
                        </div>
                    })
                } else {
                    None
                }
            }}

            <div class="ai-mode-select">
                <label class="ai-radio">
                    <input
                        type="radio"
                        name="ai-mode"
                        checked=move || mode.get() == AiMode::Pixelate
                        on:change=move |_| set_mode.set(AiMode::Pixelate)
                    />
                    " Pixelate"
                </label>
                <label class="ai-radio">
                    <input
                        type="radio"
                        name="ai-mode"
                        checked=move || mode.get() == AiMode::Palette
                        on:change=move |_| set_mode.set(AiMode::Palette)
                    />
                    " Palette"
                </label>
            </div>

            {move || {
                let m = mode.get();
                match m {
                    AiMode::Pixelate => Some(view! {
                        <div class="ai-params">
                            <label class="ai-param">
                                "Size:"
                                <select
                                    class="ai-select"
                                    on:change=move |ev| set_target_size.set(event_target_value(&ev))
                                >
                                    <option value="16" selected=move || target_size.get() == "16">"16x16"</option>
                                    <option value="32" selected=move || target_size.get() == "32">"32x32"</option>
                                    <option value="48" selected=move || target_size.get() == "48">"48x48"</option>
                                    <option value="64" selected=move || target_size.get() == "64">"64x64"</option>
                                    <option value="128" selected=move || target_size.get() == "128">"128x128"</option>
                                </select>
                            </label>
                            <label class="ai-param">
                                "Colors:"
                                <select
                                    class="ai-select"
                                    on:change=move |ev| set_max_colors.set(event_target_value(&ev))
                                >
                                    <option value="4" selected=move || max_colors.get() == "4">"4"</option>
                                    <option value="8" selected=move || max_colors.get() == "8">"8"</option>
                                    <option value="16" selected=move || max_colors.get() == "16">"16"</option>
                                    <option value="32" selected=move || max_colors.get() == "32">"32"</option>
                                    <option value="64" selected=move || max_colors.get() == "64">"64"</option>
                                </select>
                            </label>
                            <label class="ai-param">
                                "Dither:"
                                <select
                                    class="ai-select"
                                    on:change=move |ev| {
                                        let v = event_target_value(&ev);
                                        set_dither.set(match v.as_str() {
                                            "floyd" => DitherMethod::FloydSteinberg,
                                            _ => DitherMethod::None,
                                        });
                                    }
                                >
                                    <option value="none">"None"</option>
                                    <option value="floyd">"Floyd-Steinberg"</option>
                                </select>
                            </label>
                        </div>
                    }.into_any()),
                    AiMode::Palette => Some(view! {
                        <div class="ai-params">
                            <label class="ai-param">
                                "Colors:"
                                <select
                                    class="ai-select"
                                    on:change=move |ev| set_max_colors.set(event_target_value(&ev))
                                >
                                    <option value="4">"4"</option>
                                    <option value="8">"8"</option>
                                    <option value="16" selected=true>"16"</option>
                                    <option value="32">"32"</option>
                                </select>
                            </label>
                        </div>
                    }.into_any()),
                }
            }}

            <button
                class="ai-execute-btn"
                on:click=on_execute
                prop:disabled=move || ai_result.get().status == AiStatus::Loading
            >
                {move || {
                    let status = ai_result.get().status;
                    if status == AiStatus::Loading { "Processing..." } else { "Execute" }
                }}
            </button>

            // Results section
            {move || {
                let result = ai_result.get();
                match result.status {
                    AiStatus::Idle => None,
                    AiStatus::Loading => Some(view! {
                        <div class="ai-result">
                            <p class="ai-loading">"Processing..."</p>
                        </div>
                    }.into_any()),
                    AiStatus::Success => {
                        let palette = result.palette_hex.clone();
                        let comment = result.style_comment.clone();
                        Some(view! {
                            <div class="ai-result">
                                {if !comment.is_empty() {
                                    Some(view! { <p class="ai-comment">{comment}</p> })
                                } else {
                                    None
                                }}
                                {if !palette.is_empty() {
                                    let palette_for_apply = palette.clone();
                                    Some(view! {
                                        <div class="ai-palette-result">
                                            <div class="palette-grid">
                                                {palette.iter().map(|hex| {
                                                    view! {
                                                        <div
                                                            class="palette-swatch"
                                                            style=format!(
                                                                "background: {}; width: 20px; height: 20px; border: 2px solid #2a2a5a;",
                                                                hex,
                                                            )
                                                        />
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                            <button
                                                class="ai-apply-btn"
                                                on:click=move |_| on_apply_palette.run(palette_for_apply.clone())
                                            >
                                                "Apply Palette"
                                            </button>
                                        </div>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        }.into_any())
                    }
                    AiStatus::Error(ref msg) => {
                        let msg = msg.clone();
                        Some(view! {
                            <div class="ai-result ai-error">
                                <p>{msg}</p>
                            </div>
                        }.into_any())
                    }
                    AiStatus::Offline => Some(view! {
                        <div class="ai-result ai-offline">
                            <p>"AI features require an internet connection."</p>
                        </div>
                    }.into_any()),
                }
            }}
        </div>
    }
}
