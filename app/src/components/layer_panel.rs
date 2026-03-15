use leptos::prelude::*;
use pxlot_core::BlendMode;

/// Layer info for display (extracted from Canvas to avoid borrow issues).
#[derive(Clone, Debug)]
pub struct LayerInfo {
    pub index: usize,
    pub name: String,
    pub visible: bool,
    #[allow(dead_code)]
    pub locked: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
}

#[component]
pub fn LayerPanel(
    layers: ReadSignal<Vec<LayerInfo>>,
    active_layer: ReadSignal<usize>,
    on_select: Callback<usize>,
    on_add: Callback<()>,
    on_remove: Callback<usize>,
    on_toggle_visibility: Callback<usize>,
    on_opacity_change: Callback<(usize, u8)>,
    on_move_up: Callback<usize>,
    on_move_down: Callback<usize>,
    on_blend_mode_change: Callback<(usize, BlendMode)>,
) -> impl IntoView {
    view! {
        <div class="panel-section layer-panel-section" role="region" aria-label="Layers">
            <div class="panel-header">
                <h3>"Layers"</h3>
                <button class="panel-btn" title="Add Layer" aria-label="Add Layer" on:click=move |_| on_add.run(())>
                    "+"
                </button>
            </div>
            <div class="layer-list">
                <For
                    each=move || {
                        let l = layers.get();
                        l.into_iter().rev().collect::<Vec<_>>()
                    }
                    key=|info| (info.index, info.name.clone(), info.visible, info.opacity)
                    children=move |info| {
                        let idx = info.index;
                        let is_active = move || active_layer.get() == idx;
                        let name = info.name.clone();
                        let visible = info.visible;
                        let opacity = info.opacity;
                        let blend_mode = info.blend_mode;
                        let blend_mode_str = match blend_mode {
                            BlendMode::Normal => "normal",
                            BlendMode::Multiply => "multiply",
                            BlendMode::Screen => "screen",
                            BlendMode::Overlay => "overlay",
                        };
                        view! {
                            <div
                                class="layer-item"
                                class:layer-active=is_active
                                on:click=move |_| on_select.run(idx)
                            >
                                <button
                                    class="layer-vis-btn"
                                    title=if visible { "Hide" } else { "Show" }
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        on_toggle_visibility.run(idx);
                                    }
                                >
                                    {if visible { "V" } else { "-" }}
                                </button>
                                <span class="layer-name">{name}</span>
                                <select
                                    class="layer-blend-mode"
                                    title="Blend Mode"
                                    on:change=move |ev| {
                                        ev.stop_propagation();
                                        let mode = match event_target_value(&ev).as_str() {
                                            "multiply" => BlendMode::Multiply,
                                            "screen" => BlendMode::Screen,
                                            "overlay" => BlendMode::Overlay,
                                            _ => BlendMode::Normal,
                                        };
                                        on_blend_mode_change.run((idx, mode));
                                    }
                                    on:click=move |ev| ev.stop_propagation()
                                >
                                    <option value="normal" selected=move || blend_mode_str == "normal">"N"</option>
                                    <option value="multiply" selected=move || blend_mode_str == "multiply">"Mul"</option>
                                    <option value="screen" selected=move || blend_mode_str == "screen">"Scr"</option>
                                    <option value="overlay" selected=move || blend_mode_str == "overlay">"Ovr"</option>
                                </select>
                                <input
                                    type="range"
                                    class="layer-opacity"
                                    min="0"
                                    max="255"
                                    prop:value=move || opacity.to_string()
                                    on:input=move |ev| {
                                        ev.stop_propagation();
                                        if let Ok(val) = event_target_value(&ev).parse::<u8>() {
                                            on_opacity_change.run((idx, val));
                                        }
                                    }
                                    aria-label="Layer opacity"
                                />
                                <button
                                    class="layer-reorder-btn"
                                    title="Move Up"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        on_move_up.run(idx);
                                    }
                                >
                                    "\u{25b2}"
                                </button>
                                <button
                                    class="layer-reorder-btn"
                                    title="Move Down"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        on_move_down.run(idx);
                                    }
                                >
                                    "\u{25bc}"
                                </button>
                                <button
                                    class="layer-del-btn"
                                    title="Delete"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        on_remove.run(idx);
                                    }
                                >
                                    "x"
                                </button>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}
