use leptos::prelude::*;

use crate::project_api::{self, ProjectMeta};

/// Project list panel — shows saved projects with thumbnail, metadata, and actions.
#[component]
pub fn ProjectList(
    /// Whether the panel is open.
    is_open: ReadSignal<bool>,
    /// Close the panel.
    on_close: Callback<()>,
    /// Open a project by ID.
    on_open: Callback<String>,
) -> impl IntoView {
    let (projects, set_projects) = signal(Vec::<ProjectMeta>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    // Load projects when panel opens
    let load_projects = move || {
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match project_api::list_projects().await {
                Ok(list) => set_projects.set(list),
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    };

    // Reload when opened
    Effect::new(move |_| {
        if is_open.get() {
            load_projects();
        }
    });

    let do_delete = move |id: String| {
        let set_projects = set_projects.clone();
        let set_error = set_error.clone();
        leptos::task::spawn_local(async move {
            match project_api::delete_project(&id).await {
                Ok(()) => {
                    set_projects.update(|list| list.retain(|p| p.id != id));
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    view! {
        <div
            class="project-list-overlay"
            style:display=move || if is_open.get() { "flex" } else { "none" }
        >
            <div class="project-list-panel">
                <div class="project-list-header">
                    <h2>"Projects"</h2>
                    <button class="menu-btn" on:click=move |_| on_close.run(())>"X"</button>
                </div>

                {move || error.get().map(|e| view! {
                    <p class="project-error">{e}</p>
                })}

                {move || loading.get().then(|| view! {
                    <p class="project-loading">"Loading..."</p>
                })}

                <div class="project-grid">
                    {move || {
                        projects.get().into_iter().map(|p| {
                            let id_open = p.id.clone();
                            let id_delete = p.id.clone();
                            let on_open = on_open.clone();
                            let do_delete = do_delete.clone();

                            view! {
                                <div class="project-card">
                                    <div class="project-thumb">
                                        {if let Some(ref thumb) = p.thumbnail {
                                            view! {
                                                <img src={format!("data:image/png;base64,{thumb}")} alt="thumbnail" />
                                            }.into_any()
                                        } else {
                                            view! { <div class="project-thumb-empty"></div> }.into_any()
                                        }}
                                    </div>
                                    <div class="project-info">
                                        <span class="project-name">{p.name.clone()}</span>
                                        <span class="project-meta">
                                            {format!("{}x{}", p.width, p.height)}
                                            " · "
                                            {format!("{} frames", p.frame_count)}
                                        </span>
                                    </div>
                                    <div class="project-actions">
                                        <button
                                            class="menu-btn project-open-btn"
                                            on:click=move |_| on_open.run(id_open.clone())
                                        >
                                            "Open"
                                        </button>
                                        <button
                                            class="menu-btn project-delete-btn"
                                            on:click=move |_| do_delete(id_delete.clone())
                                        >
                                            "Del"
                                        </button>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}
                </div>

                {move || {
                    (!loading.get() && projects.get().is_empty()).then(|| view! {
                        <p class="project-empty">"No saved projects yet."</p>
                    })
                }}
            </div>
        </div>
    }
}
