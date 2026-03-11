use leptos::prelude::*;

#[component]
pub fn TimelinePanel(
    frame_count: ReadSignal<usize>,
    current_frame: ReadSignal<usize>,
    fps: ReadSignal<u32>,
    playing: ReadSignal<bool>,
    onion_skin: ReadSignal<bool>,
    onion_skin_frames: ReadSignal<u32>,
    on_select_frame: Callback<usize>,
    on_add_frame: Callback<()>,
    on_duplicate_frame: Callback<()>,
    on_remove_frame: Callback<()>,
    on_prev_frame: Callback<()>,
    on_next_frame: Callback<()>,
    on_toggle_play: Callback<()>,
    on_fps_change: Callback<u32>,
    on_toggle_onion_skin: Callback<()>,
    on_onion_skin_frames_change: Callback<u32>,
) -> impl IntoView {
    view! {
        <div class="timeline-panel" role="region" aria-label="Animation Timeline">
            <div class="timeline-controls">
                <button
                    class="timeline-btn"
                    title="Previous Frame"
                    on:click=move |_| on_prev_frame.run(())
                    aria-label="Previous Frame"
                >
                    "\u{25C0}"
                </button>
                <button
                    class="timeline-btn play-btn"
                    title="Play/Pause"
                    on:click=move |_| on_toggle_play.run(())
                    aria-label="Play or Pause"
                >
                    {move || if playing.get() { "\u{25A0}" } else { "\u{25B6}" }}
                </button>
                <button
                    class="timeline-btn"
                    title="Next Frame"
                    on:click=move |_| on_next_frame.run(())
                    aria-label="Next Frame"
                >
                    "\u{25B6}"
                </button>
                <span class="timeline-info">
                    {move || format!("{}/{}", current_frame.get() + 1, frame_count.get())}
                </span>
                <label class="timeline-fps" aria-label="Frames per second">
                    "FPS:"
                    <input
                        type="number"
                        class="fps-input"
                        min="1"
                        max="60"
                        prop:value=move || fps.get().to_string()
                        on:change=move |ev| {
                            if let Ok(val) = event_target_value(&ev).parse::<u32>() {
                                on_fps_change.run(val.clamp(1, 60));
                            }
                        }
                    />
                </label>
                <span class="onion-skin-separator">"|"</span>
                <label class="onion-skin-toggle" title="Onion Skin">
                    <input
                        type="checkbox"
                        prop:checked=move || onion_skin.get()
                        on:change=move |_| on_toggle_onion_skin.run(())
                    />
                    <span class="onion-skin-label">"Onion"</span>
                </label>
                <select
                    class="onion-frames-select"
                    title="Onion skin frames"
                    prop:value=move || onion_skin_frames.get().to_string()
                    on:change=move |ev| {
                        if let Ok(val) = event_target_value(&ev).parse::<u32>() {
                            on_onion_skin_frames_change.run(val.clamp(1, 4));
                        }
                    }
                >
                    <option value="1" selected=move || onion_skin_frames.get() == 1>"1"</option>
                    <option value="2" selected=move || onion_skin_frames.get() == 2>"2"</option>
                    <option value="3" selected=move || onion_skin_frames.get() == 3>"3"</option>
                    <option value="4" selected=move || onion_skin_frames.get() == 4>"4"</option>
                </select>
                <span class="onion-skin-separator">"|"</span>
                <button
                    class="timeline-btn"
                    title="Add Frame"
                    on:click=move |_| on_add_frame.run(())
                    aria-label="Add Frame"
                >
                    "+"
                </button>
                <button
                    class="timeline-btn"
                    title="Duplicate Frame"
                    on:click=move |_| on_duplicate_frame.run(())
                    aria-label="Duplicate Frame"
                >
                    "D"
                </button>
                <button
                    class="timeline-btn"
                    title="Delete Frame"
                    on:click=move |_| on_remove_frame.run(())
                    aria-label="Delete Frame"
                >
                    "x"
                </button>
            </div>
            <div class="timeline-frames">
                {move || {
                    let count = frame_count.get();
                    let cur = current_frame.get();
                    (0..count)
                        .map(|i| {
                            let is_active = i == cur;
                            view! {
                                <button
                                    class="frame-thumb"
                                    class:frame-active=is_active
                                    on:click=move |_| on_select_frame.run(i)
                                    aria-label=format!("Frame {}", i + 1)
                                >
                                    {i + 1}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}
