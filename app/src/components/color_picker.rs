use leptos::prelude::*;
use pxlot_core::Color;

/// Default palette colors.
const DEFAULT_PALETTE: &[&str] = &[
    "#000000", "#ffffff", "#ff0000", "#00ff00", "#0000ff", "#ffff00",
    "#ff00ff", "#00ffff", "#ff8800", "#8800ff", "#0088ff", "#88ff00",
    "#ff0088", "#00ff88", "#880000", "#008800", "#000088", "#888888",
    "#444444", "#cccccc", "#ff4444", "#44ff44", "#4444ff", "#ffaa00",
    "#aa00ff", "#00aaff", "#ffcc88", "#88ccff", "#cc88ff", "#88ffcc",
    "#553300", "#335500",
];

#[component]
pub fn ColorPicker(
    current_color: ReadSignal<Color>,
    set_color: WriteSignal<Color>,
    color_history: ReadSignal<Vec<Color>>,
) -> impl IntoView {
    let hex_input = RwSignal::new(current_color.get_untracked().to_hex());

    // Sync hex_input when current_color changes externally
    Effect::new(move |_| {
        hex_input.set(current_color.get().to_hex());
    });

    let (hex_valid, set_hex_valid) = signal(true);

    let on_hex_submit = move |_| {
        let val = hex_input.get();
        if let Some(c) = Color::from_hex(&val) {
            set_color.set(c);
            set_hex_valid.set(true);
        } else {
            set_hex_valid.set(false);
        }
    };

    view! {
        <div class="panel-section" role="region" aria-label="Color Picker">
            <h3>"Color"</h3>
            <div
                class="color-preview"
                style=move || {
                    format!(
                        "background: {}; width: 48px; height: 48px; border: 2px solid #3a3a7a;",
                        current_color.get().to_css(),
                    )
                }
                aria-label=move || format!("Current color: {}", current_color.get().to_hex())
            ></div>
            <div class="color-hex-input">
                <input
                    type="text"
                    class="hex-input"
                    class:hex-invalid=move || !hex_valid.get()
                    prop:value=move || hex_input.get()
                    on:input=move |ev| {
                        hex_input.set(event_target_value(&ev));
                        set_hex_valid.set(true);
                    }
                    on:change=on_hex_submit
                    maxlength="7"
                    placeholder="#000000"
                    aria-label="Hex color input"
                />
            </div>
            <div class="color-rgb-sliders">
                <label class="rgb-slider">
                    "R"
                    <input
                        type="range"
                        min="0" max="255"
                        prop:value=move || current_color.get().r.to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                let c = current_color.get();
                                set_color.set(Color::new(v, c.g, c.b, c.a));
                            }
                        }
                        aria-label="Red"
                    />
                </label>
                <label class="rgb-slider">
                    "G"
                    <input
                        type="range"
                        min="0" max="255"
                        prop:value=move || current_color.get().g.to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                let c = current_color.get();
                                set_color.set(Color::new(c.r, v, c.b, c.a));
                            }
                        }
                        aria-label="Green"
                    />
                </label>
                <label class="rgb-slider">
                    "B"
                    <input
                        type="range"
                        min="0" max="255"
                        prop:value=move || current_color.get().b.to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u8>() {
                                let c = current_color.get();
                                set_color.set(Color::new(c.r, c.g, v, c.a));
                            }
                        }
                        aria-label="Blue"
                    />
                </label>
            </div>
            <div class="palette-grid" role="group" aria-label="Color palette">
                {DEFAULT_PALETTE
                    .iter()
                    .map(|&hex| {
                        let color = Color::from_hex(hex).unwrap_or(Color::BLACK);
                        let is_selected = move || current_color.get() == color;
                        view! {
                            <button
                                class="palette-swatch"
                                class:palette-selected=is_selected
                                style=format!(
                                    "background: {}; width: 20px; height: 20px; border: 2px solid #2a2a5a; cursor: pointer; padding: 0;",
                                    hex,
                                )
                                on:click=move |_| set_color.set(color)
                                aria-label=hex
                            />
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
            <div class="color-history" role="group" aria-label="Recent colors">
                {move || {
                    color_history
                        .get()
                        .into_iter()
                        .map(|color| {
                            let css = color.to_css();
                            let hex = color.to_hex();
                            view! {
                                <button
                                    class="color-history-swatch"
                                    style=format!(
                                        "background: {};",
                                        css,
                                    )
                                    on:click=move |_| set_color.set(color)
                                    aria-label=hex
                                />
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}
