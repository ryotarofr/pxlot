use leptos::prelude::*;

use crate::auth;
use crate::project_api::{self, ProjectMeta};

/// Describes what action the user chose on the project list page.
#[derive(Clone)]
pub enum ProjectAction {
    /// Open an existing project (by ID).
    Open(String),
    /// Create a new blank project with the given dimensions.
    New { width: u32, height: u32 },
}

/// Full-screen project list page — shown after login, before the editor.
#[component]
pub fn ProjectListPage(
    /// Callback when the user picks an action (open or new).
    on_action: Callback<ProjectAction>,
    /// Callback to log out.
    on_logout: Callback<()>,
) -> impl IntoView {
    let (projects, set_projects) = signal(Vec::<ProjectMeta>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(Option::<String>::None);
    let (show_new_dialog, set_show_new_dialog) = signal(false);
    let (new_width, set_new_width) = signal("32".to_string());
    let (new_height, set_new_height) = signal("32".to_string());
    // Delete confirmation
    let (delete_target, set_delete_target) = signal(Option::<(String, String)>::None);

    // Load projects on mount
    leptos::task::spawn_local(async move {
        match project_api::list_projects().await {
            Ok(list) => set_projects.set(list),
            Err(e) => set_error.set(Some(e)),
        }
        set_loading.set(false);
    });

    // Delete handler
    let do_delete = move |id: String| {
        leptos::task::spawn_local(async move {
            match project_api::delete_project(&id).await {
                Ok(()) => {
                    set_projects.update(|list| list.retain(|p| p.id != id));
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    // Get user info for display
    let user = auth::load_user();
    let user_name = user.as_ref().map(|u| u.name.clone()).unwrap_or_default();
    let user_avatar = user.as_ref().and_then(|u| u.avatar_url.clone());

    view! {
        <div class="project-page">
            <header class="project-page-header">
                <span class="project-page-title">"pxlot"</span>
                <div class="project-page-user">
                    <a class="menu-btn" href="/pxlot/gallery">"Gallery"</a>
                    {user_avatar.map(|url| view! {
                        <img class="project-page-avatar" src=url alt="avatar" />
                    })}
                    <span class="project-page-username">{user_name}</span>
                    <button class="menu-btn" on:click=move |_| on_logout.run(())>"Logout"</button>
                </div>
            </header>

            <div class="project-page-body">
                <div class="project-page-toolbar">
                    <h2>"My Projects"</h2>
                    <button
                        class="menu-btn project-new-btn"
                        on:click=move |_| set_show_new_dialog.set(true)
                    >
                        "+ New Project"
                    </button>
                </div>

                {move || error.get().map(|e| view! {
                    <p class="project-error">{e}</p>
                })}

                {move || loading.get().then(|| view! {
                    <p class="project-loading">"Loading projects..."</p>
                })}

                <div class="project-page-grid">
                    {move || {
                        projects.get().into_iter().map(|p| {
                            let id_open = p.id.clone();
                            let id_delete = p.id.clone();
                            let delete_name = p.name.clone();
                            let on_action = on_action.clone();

                            view! {
                                <div
                                    class="project-page-card"
                                    on:dblclick=move |_| on_action.run(ProjectAction::Open(id_open.clone()))
                                >
                                    <div class="project-page-thumb">
                                        {if let Some(ref thumb) = p.thumbnail {
                                            view! {
                                                <img src={format!("data:image/png;base64,{thumb}")} alt="thumbnail" />
                                            }.into_any()
                                        } else {
                                            view! { <div class="project-thumb-placeholder"></div> }.into_any()
                                        }}
                                    </div>
                                    <div class="project-page-card-info">
                                        <span class="project-page-card-name">{p.name.clone()}</span>
                                        <span class="project-page-card-meta">
                                            {format!("{}x{} · {} frames", p.width, p.height, p.frame_count)}
                                        </span>
                                    </div>
                                    <div class="project-page-card-actions">
                                        <button
                                            class="menu-btn project-open-btn"
                                            on:click={
                                                let id = p.id.clone();
                                                let on_action = on_action.clone();
                                                move |_| on_action.run(ProjectAction::Open(id.clone()))
                                            }
                                        >
                                            "Open"
                                        </button>
                                        <button
                                            class="menu-btn project-delete-btn"
                                            on:click=move |_| set_delete_target.set(Some((id_delete.clone(), delete_name.clone())))
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
                        <div class="project-page-empty">
                            <p>"No projects yet."</p>
                            <p>"Click \"+ New Project\" to get started."</p>
                        </div>
                    })
                }}
            </div>

            // New project dialog
            {move || {
                show_new_dialog.get().then(|| view! {
                    <div class="modal-overlay" on:click=move |_| set_show_new_dialog.set(false)>
                        <div class="modal-dialog" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                            <h3>"New Project"</h3>
                            <label>"Width: "
                                <input type="number" min="1" max="256"
                                    prop:value=move || new_width.get()
                                    on:input=move |ev| set_new_width.set(event_target_value(&ev))
                                />
                            </label>
                            <label>"Height: "
                                <input type="number" min="1" max="256"
                                    prop:value=move || new_height.get()
                                    on:input=move |ev| set_new_height.set(event_target_value(&ev))
                                />
                            </label>
                            <div class="modal-preset-btns">
                                <button class="menu-btn" on:click=move |_| { set_new_width.set("16".into()); set_new_height.set("16".into()); }>"16x16"</button>
                                <button class="menu-btn" on:click=move |_| { set_new_width.set("32".into()); set_new_height.set("32".into()); }>"32x32"</button>
                                <button class="menu-btn" on:click=move |_| { set_new_width.set("64".into()); set_new_height.set("64".into()); }>"64x64"</button>
                                <button class="menu-btn" on:click=move |_| { set_new_width.set("128".into()); set_new_height.set("128".into()); }>"128x128"</button>
                            </div>
                            <div class="modal-actions">
                                <button class="menu-btn" on:click=move |_| {
                                    let w: u32 = new_width.get().parse().unwrap_or(32).clamp(1, 256);
                                    let h: u32 = new_height.get().parse().unwrap_or(32).clamp(1, 256);
                                    set_show_new_dialog.set(false);
                                    on_action.run(ProjectAction::New { width: w, height: h });
                                }>"Create"</button>
                                <button class="menu-btn" on:click=move |_| set_show_new_dialog.set(false)>"Cancel"</button>
                            </div>
                        </div>
                    </div>
                })
            }}

            // Delete confirmation dialog
            {move || {
                delete_target.get().map(|(id, name)| {
                    let id_confirm = id.clone();
                    view! {
                        <div class="modal-overlay" on:click=move |_| set_delete_target.set(None)>
                            <div class="modal-dialog modal-delete-confirm" on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()>
                                <h3>"Delete Project"</h3>
                                <p class="modal-delete-msg">
                                    {format!("Delete \"{}\"?", name)}
                                </p>
                                <p class="modal-delete-warn">"This action cannot be undone."</p>
                                <div class="modal-actions">
                                    <button
                                        class="menu-btn project-delete-btn"
                                        on:click=move |_| {
                                            let id = id_confirm.clone();
                                            set_delete_target.set(None);
                                            do_delete(id);
                                        }
                                    >
                                        "Delete"
                                    </button>
                                    <button class="menu-btn" on:click=move |_| set_delete_target.set(None)>"Cancel"</button>
                                </div>
                            </div>
                        </div>
                    }
                })
            }}
        </div>
    }
}
