//! Client-side API for project CRUD operations against the server.
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::auth;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub is_public: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFull {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub is_public: Option<bool>,
    pub data: serde_json::Value,
}

#[derive(Serialize)]
pub struct SaveProjectRequest {
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub is_public: Option<bool>,
    pub frame_thumbnails: Option<serde_json::Value>,
    pub data: serde_json::Value,
}

/// Gallery item returned by the public gallery API (list).
#[derive(Debug, Clone, Deserialize)]
pub struct GalleryItem {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub created_at: String,
    pub author_name: String,
    pub author_avatar: Option<String>,
}

/// Gallery detail with frame thumbnails.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GalleryDetailItem {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub frame_thumbnails: Option<Vec<String>>,
    pub created_at: String,
    pub author_name: String,
    pub author_avatar: Option<String>,
}

/// List all projects for the current user.
pub async fn list_projects() -> Result<Vec<ProjectMeta>, String> {
    let resp = authed_fetch("GET", "/api/projects", None).await?;
    parse_json(&resp).await
}

/// Get a full project by ID.
pub async fn get_project(id: &str) -> Result<ProjectFull, String> {
    let resp = authed_fetch("GET", &format!("/api/projects/{id}"), None).await?;
    parse_json(&resp).await
}

/// Create a new project.
pub async fn create_project(req: &SaveProjectRequest) -> Result<ProjectMeta, String> {
    let body = serde_json::to_string(req).map_err(|e| format!("{e}"))?;
    let resp = authed_fetch("POST", "/api/projects", Some(&body)).await?;
    parse_json(&resp).await
}

/// Update an existing project.
pub async fn update_project(id: &str, req: &SaveProjectRequest) -> Result<ProjectMeta, String> {
    let body = serde_json::to_string(req).map_err(|e| format!("{e}"))?;
    let resp = authed_fetch("PUT", &format!("/api/projects/{id}"), Some(&body)).await?;
    parse_json(&resp).await
}

/// Delete a project.
pub async fn delete_project(id: &str) -> Result<(), String> {
    let resp = authed_fetch("DELETE", &format!("/api/projects/{id}"), None).await?;
    let r: web_sys::Response = resp.dyn_into().map_err(|e| format!("{e:?}"))?;
    if !r.ok() {
        return Err(format!("Delete failed: {}", r.status()));
    }
    Ok(())
}

/// Fetch public gallery items (no auth required).
pub async fn list_gallery() -> Result<Vec<GalleryItem>, String> {
    let resp = unauthenticated_fetch("GET", "/api/gallery").await?;
    parse_json(&resp).await
}

/// Fetch a single gallery item with frame thumbnails (no auth required).
pub async fn get_gallery_detail(id: &str) -> Result<GalleryDetailItem, String> {
    let resp = unauthenticated_fetch("GET", &format!("/api/gallery/{id}")).await?;
    parse_json(&resp).await
}

// ── Helpers ──────────────────────────────────────────────────

async fn authed_fetch(method: &str, url: &str, body: Option<&str>) -> Result<JsValue, String> {
    let token = auth::load_token().ok_or("Not authenticated")?;
    let window = web_sys::window().ok_or("No window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method(method);
    if let Some(b) = body {
        opts.set_body(&JsValue::from_str(b));
    }

    let request =
        web_sys::Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{e:?}"))?;
    request
        .headers()
        .set("Authorization", &format!("Bearer {token}"))
        .map_err(|e| format!("{e:?}"))?;
    if body.is_some() {
        request
            .headers()
            .set("Content-Type", "application/json")
            .map_err(|e| format!("{e:?}"))?;
    }

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;

    Ok(resp)
}

async fn unauthenticated_fetch(method: &str, url: &str) -> Result<JsValue, String> {
    let window = web_sys::window().ok_or("No window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method(method);

    let request =
        web_sys::Request::new_with_str_and_init(url, &opts).map_err(|e| format!("{e:?}"))?;

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;

    Ok(resp)
}

async fn parse_json<T: for<'de> Deserialize<'de>>(resp: &JsValue) -> Result<T, String> {
    let r: &web_sys::Response = resp.unchecked_ref();

    let text = wasm_bindgen_futures::JsFuture::from(r.text().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;

    let text = text.as_string().ok_or("Response not a string")?;

    if !r.ok() {
        return Err(format!("API error ({}): {text}", r.status()));
    }

    serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))
}
