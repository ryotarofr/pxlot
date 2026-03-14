use leptos::prelude::*;

use crate::project_api::{self, GalleryItem, GalleryDetailItem};

/// A single gallery card with play/pause toggle for animations.
#[component]
fn GalleryCard(
    item: GalleryItem,
    on_select: Callback<GalleryItem>,
) -> impl IntoView {
    let has_animation = item.frame_count > 1 && item.thumbnail_gif.is_some();
    let (playing, set_playing) = signal(true);

    let gif_src = item.thumbnail_gif.clone().map(|g| format!("data:image/gif;base64,{g}"));
    let png_src = item.thumbnail.clone().map(|t| format!("data:image/png;base64,{t}"));

    let item_for_click = item.clone();

    view! {
        <div class="gallery-card">
            <div
                class="gallery-card-thumb"
                class:gallery-card-thumb-clickable=has_animation
                on:click=move |_| {
                    if has_animation {
                        set_playing.update(|p| *p = !*p);
                    }
                }
            >
                {if has_animation {
                    let gif = gif_src.clone().unwrap();
                    let png = png_src.clone().unwrap_or_default();
                    view! {
                        <img
                            src=move || if playing.get() { gif.clone() } else { png.clone() }
                            alt={item.name.clone()}
                        />
                        <div class="gallery-play-badge">
                            {move || if playing.get() { "\u{25A0}" } else { "\u{25B6}" }}
                        </div>
                    }.into_any()
                } else if let Some(ref src) = png_src {
                    view! {
                        <img src={src.clone()} alt={item.name.clone()} />
                    }.into_any()
                } else {
                    view! {
                        <div class="gallery-thumb-placeholder"></div>
                    }.into_any()
                }}
            </div>
            <div
                class="gallery-card-info"
                on:click={
                    let item = item_for_click.clone();
                    move |_| on_select.run(item.clone())
                }
            >
                <span class="gallery-card-title">{item.name.clone()}</span>
                <div class="gallery-card-meta">
                    <div class="gallery-card-author">
                        {item.author_avatar.clone().map(|url| view! {
                            <img class="gallery-author-avatar" src=url alt="avatar" />
                        })}
                        <span class="gallery-author-name">{item.author_name.clone()}</span>
                    </div>
                    <span class="gallery-card-size">
                        {format!("{}x{}", item.width, item.height)}
                        {if item.frame_count > 1 {
                            format!(" · {} frames", item.frame_count)
                        } else {
                            String::new()
                        }}
                    </span>
                </div>
            </div>
        </div>
    }
}

/// Detail modal for a gallery item — fetches frame thumbnails from API.
#[component]
fn GalleryDetail(
    item: GalleryItem,
    on_close: Callback<()>,
) -> impl IntoView {
    let has_animation = item.frame_count > 1 && item.thumbnail_gif.is_some();
    let (playing, set_playing) = signal(true);
    let (current_frame, set_current_frame) = signal(0usize);
    let (detail, set_detail) = signal(Option::<GalleryDetailItem>::None);
    let (loading_frames, set_loading_frames) = signal(false);

    let gif_src = item.thumbnail_gif.clone().map(|g| format!("data:image/gif;base64,{g}"));
    let png_src = item.thumbnail.clone().map(|t| format!("data:image/png;base64,{t}"));
    let frame_count = item.frame_count;

    // Fetch detail (with frame_thumbnails) if animated
    if item.frame_count > 1 {
        let id = item.id.clone();
        set_loading_frames.set(true);
        leptos::task::spawn_local(async move {
            match project_api::get_gallery_detail(&id).await {
                Ok(d) => set_detail.set(Some(d)),
                Err(e) => log::error!("Failed to load gallery detail: {e}"),
            }
            set_loading_frames.set(false);
        });
    }

    view! {
        <div class="gallery-detail-overlay" on:click=move |_| on_close.run(())>
            <div class="gallery-detail" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                <button class="gallery-detail-close" on:click=move |_| on_close.run(())>"\u{2715}"</button>

                <div
                    class="gallery-detail-image"
                    class:gallery-card-thumb-clickable=has_animation
                    on:click=move |_| {
                        if has_animation {
                            set_playing.update(|p| *p = !*p);
                            set_current_frame.set(0);
                        }
                    }
                >
                    {move || {
                        let frame_idx = current_frame.get();
                        let is_playing = playing.get();

                        if !is_playing && frame_count > 1 {
                            // Show specific frame from frame_thumbnails
                            if let Some(ref d) = detail.get() {
                                if let Some(ref thumbs) = d.frame_thumbnails {
                                    if let Some(b64) = thumbs.get(frame_idx) {
                                        return view! {
                                            <img
                                                src={format!("data:image/png;base64,{b64}")}
                                                alt={format!("Frame {}", frame_idx + 1)}
                                            />
                                        }.into_any();
                                    }
                                }
                            }
                            if let Some(ref src) = png_src {
                                return view! { <img src={src.clone()} alt="thumbnail" /> }.into_any();
                            }
                            view! { <div class="gallery-thumb-placeholder"></div> }.into_any()
                        } else if has_animation {
                            let gif = gif_src.clone().unwrap();
                            let png = png_src.clone().unwrap_or_default();
                            view! {
                                <img
                                    src=move || if playing.get() { gif.clone() } else { png.clone() }
                                    alt="thumbnail"
                                />
                                <div class="gallery-play-badge gallery-play-badge-lg">
                                    {move || if playing.get() { "\u{25A0}" } else { "\u{25B6}" }}
                                </div>
                            }.into_any()
                        } else if let Some(ref src) = png_src {
                            view! { <img src={src.clone()} alt="thumbnail" /> }.into_any()
                        } else {
                            view! { <div class="gallery-thumb-placeholder"></div> }.into_any()
                        }
                    }}
                </div>

                // Frame selector (only for multi-frame projects)
                {(frame_count > 1).then(|| view! {
                    <div class="gallery-frame-selector">
                        {move || {
                            if loading_frames.get() {
                                return view! { <span class="gallery-frame-loading">"Loading frames..."</span> }.into_any();
                            }
                            let thumbs = detail.get().and_then(|d| d.frame_thumbnails);
                            if let Some(thumbs) = thumbs {
                                view! {
                                    <div class="gallery-frame-strip">
                                        {thumbs.into_iter().enumerate().map(|(i, b64)| {
                                            let is_selected = move || !playing.get() && current_frame.get() == i;
                                            view! {
                                                <button
                                                    class="gallery-frame-btn"
                                                    class:gallery-frame-btn-active=is_selected
                                                    on:click=move |_| {
                                                        set_playing.set(false);
                                                        set_current_frame.set(i);
                                                    }
                                                    title={format!("Frame {}", i + 1)}
                                                >
                                                    <img src={format!("data:image/png;base64,{b64}")} alt={format!("Frame {}", i + 1)} />
                                                    <span class="gallery-frame-num">{i + 1}</span>
                                                </button>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                    <button
                                        class="menu-btn gallery-play-all-btn"
                                        on:click=move |_| set_playing.set(true)
                                    >
                                        {move || if playing.get() { "Playing..." } else { "\u{25B6} Play" }}
                                    </button>
                                }.into_any()
                            } else {
                                view! { <span></span> }.into_any()
                            }
                        }}
                    </div>
                })}

                <div class="gallery-detail-info">
                    <h2 class="gallery-detail-title">{item.name.clone()}</h2>
                    <div class="gallery-detail-author">
                        {item.author_avatar.clone().map(|url| view! {
                            <img class="gallery-detail-avatar" src=url alt="avatar" />
                        })}
                        <span>{item.author_name.clone()}</span>
                    </div>
                    <div class="gallery-detail-meta">
                        <span>{format!("Size: {}x{}", item.width, item.height)}</span>
                        {(item.frame_count > 1).then(|| view! {
                            <span>{format!("Frames: {}", item.frame_count)}</span>
                        })}
                        <span>{format!("Created: {}", item.created_at.get(..10).unwrap_or(&item.created_at))}</span>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Public gallery page — shows all published pixel art.
#[component]
pub fn GalleryPage() -> impl IntoView {
    let (items, set_items) = signal(Vec::<GalleryItem>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<GalleryItem>::None);

    leptos::task::spawn_local(async move {
        match project_api::list_gallery().await {
            Ok(list) => set_items.set(list),
            Err(e) => set_error.set(Some(e)),
        }
        set_loading.set(false);
    });

    let on_select = Callback::new(move |item: GalleryItem| {
        set_selected.set(Some(item));
    });
    let on_close = Callback::new(move |_: ()| {
        set_selected.set(None);
    });

    view! {
        <div class="gallery-page">
            <header class="gallery-header">
                <div class="gallery-header-inner">
                    <h1 class="gallery-title">"pxlot Gallery"</h1>
                    <a class="gallery-app-link menu-btn" href="/pxlot/">"Open Editor"</a>
                </div>
            </header>

            <main class="gallery-body">
                {move || error.get().map(|e| view! {
                    <p class="gallery-error">{e}</p>
                })}

                {move || loading.get().then(|| view! {
                    <p class="gallery-loading">"Loading gallery..."</p>
                })}

                <div class="gallery-grid">
                    {move || {
                        items.get().into_iter().map(|item| {
                            view! { <GalleryCard item=item on_select=on_select /> }
                        }).collect::<Vec<_>>()
                    }}
                </div>

                {move || {
                    (!loading.get() && items.get().is_empty()).then(|| view! {
                        <div class="gallery-empty">
                            <p>"No published works yet."</p>
                        </div>
                    })
                }}
            </main>

            {move || {
                selected.get().map(|item| view! {
                    <GalleryDetail item=item on_close=on_close />
                })
            }}
        </div>
    }
}
