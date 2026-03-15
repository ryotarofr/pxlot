use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::auth;
use crate::db;

// ── Request / Response types ─────────────────────────────────

#[derive(Deserialize)]
pub struct GoogleAuthRequest {
    pub id_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

impl From<db::User> for UserResponse {
    fn from(u: db::User) -> Self {
        Self {
            id: u.id.to_string(),
            email: u.email,
            name: u.name,
            avatar_url: u.avatar_url,
        }
    }
}

#[derive(Deserialize)]
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

/// Validate a SaveProjectRequest, returning an error response if invalid.
fn validate_save_project(req: &SaveProjectRequest) -> Result<(), axum::response::Response> {
    if req.name.is_empty() || req.name.len() > 255 {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Name must be between 1 and 255 characters",
        ));
    }
    if req.width <= 0 || req.width > 4096 {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Width must be between 1 and 4096",
        ));
    }
    if req.height <= 0 || req.height > 4096 {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Height must be between 1 and 4096",
        ));
    }
    if req.frame_count <= 0 || req.frame_count > 1000 {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "Frame count must be between 1 and 1000",
        ));
    }
    Ok(())
}

// ── Auth handlers ────────────────────────────────────────────

/// POST /api/auth/google — authenticate with Google ID token
pub async fn google_auth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GoogleAuthRequest>,
) -> impl IntoResponse {
    let google_user =
        match auth::verify_google_id_token(&state.client, &req.id_token, &state.google_client_id)
            .await
        {
            Ok(u) => u,
            Err(e) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": e })),
                )
                    .into_response();
            }
        };

    let email = google_user.email.unwrap_or_default();
    let name = google_user.name.unwrap_or_else(|| "User".to_string());
    let avatar = google_user.picture.as_deref();

    let user = match db::upsert_user(&state.pool, &google_user.sub, &email, &name, avatar).await {
        Ok(u) => u,
        Err(e) => {
            eprintln!("DB error: {e}");
            return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error");
        }
    };

    let token = match auth::create_token(user.id, &state.jwt_secret) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("JWT error: {e}");
            return err_response(StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed");
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!(AuthResponse {
            token,
            user: user.into(),
        })),
    )
        .into_response()
}

/// GET /api/auth/me — get current user from JWT
pub async fn me(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match db::find_user_by_id(&state.pool, user_id).await {
        Ok(Some(user)) => {
            let resp: UserResponse = user.into();
            (StatusCode::OK, Json(serde_json::json!(resp))).into_response()
        }
        Ok(None) => err_response(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// GET /api/config — return public configuration
pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "google_client_id": state.google_client_id,
        })),
    )
        .into_response()
}

// ── Project handlers ─────────────────────────────────────────

/// GET /api/projects — list all projects for the authenticated user
pub async fn list_projects(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match db::list_projects(&state.pool, user_id).await {
        Ok(projects) => (StatusCode::OK, Json(serde_json::json!(projects))).into_response(),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// GET /api/projects/:id — get a single project
pub async fn get_project(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(project_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match db::get_project(&state.pool, project_id, user_id).await {
        Ok(Some(project)) => (StatusCode::OK, Json(serde_json::json!(project))).into_response(),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Project not found"),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// POST /api/projects — create a new project
pub async fn create_project(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SaveProjectRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if let Err(resp) = validate_save_project(&req) {
        return resp;
    }

    let new_project = db::NewProject {
        user_id,
        name: &req.name,
        width: req.width,
        height: req.height,
        frame_count: req.frame_count,
        thumbnail: req.thumbnail.as_deref(),
        thumbnail_gif: req.thumbnail_gif.as_deref(),
        is_public: req.is_public.unwrap_or(false),
        frame_thumbnails: req.frame_thumbnails.as_ref(),
        data: &req.data,
    };

    match db::create_project(&state.pool, new_project).await {
        Ok(meta) => (StatusCode::CREATED, Json(serde_json::json!(meta))).into_response(),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// PUT /api/projects/:id — update an existing project
pub async fn update_project(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(req): Json<SaveProjectRequest>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if let Err(resp) = validate_save_project(&req) {
        return resp;
    }

    let updated_project = db::UpdateProject {
        project_id,
        user_id,
        name: &req.name,
        width: req.width,
        height: req.height,
        frame_count: req.frame_count,
        thumbnail: req.thumbnail.as_deref(),
        thumbnail_gif: req.thumbnail_gif.as_deref(),
        is_public: req.is_public.unwrap_or(false),
        frame_thumbnails: req.frame_thumbnails.as_ref(),
        data: &req.data,
    };

    match db::update_project(&state.pool, updated_project).await {
        Ok(Some(meta)) => (StatusCode::OK, Json(serde_json::json!(meta))).into_response(),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Project not found"),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// DELETE /api/projects/:id — delete a project
pub async fn delete_project(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(project_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match db::delete_project(&state.pool, project_id, user_id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "deleted": true }))).into_response(),
        Ok(false) => err_response(StatusCode::NOT_FOUND, "Project not found"),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

// ── Gallery (public, no auth) ────────────────────────────────

/// GET /api/gallery — list all public projects with author info
pub async fn list_gallery(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::list_gallery(&state.pool).await {
        Ok(items) => (StatusCode::OK, Json(serde_json::json!(items))).into_response(),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

/// GET /api/gallery/:id — get a single public project with frame thumbnails
pub async fn get_gallery_detail(
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
) -> impl IntoResponse {
    match db::get_gallery_detail(&state.pool, project_id).await {
        Ok(Some(detail)) => (StatusCode::OK, Json(serde_json::json!(detail))).into_response(),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Project not found"),
        Err(e) => {
            eprintln!("DB error: {e}");
            err_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
        }
    }
}

// ── Image generation (Replicate API) ─────────────────────────

#[derive(Deserialize)]
pub struct GenerateImageRequest {
    pub prompt: String,
    /// Replicate model ID (e.g. "retro-diffusion/rd-plus")
    #[serde(default = "default_replicate_model")]
    pub model: String,
    #[serde(default = "default_image_width")]
    pub width: u32,
    #[serde(default = "default_image_height")]
    pub height: u32,
}

fn default_replicate_model() -> String {
    "retro-diffusion/rd-plus".to_string()
}
fn default_image_width() -> u32 {
    256
}
fn default_image_height() -> u32 {
    256
}

/// POST /api/generate-image — generate pixel art via Replicate API
pub async fn generate_image(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<GenerateImageRequest>,
) -> impl IntoResponse {
    // Authenticate
    let _user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Check Replicate token
    let replicate_token = match &state.replicate_api_token {
        Some(t) => t,
        None => {
            return err_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "REPLICATE_API_TOKEN not configured on server",
            );
        }
    };

    // Build Replicate prediction request
    let replicate_body = serde_json::json!({
        "input": {
            "prompt": req.prompt,
            "width": req.width,
            "height": req.height,
        }
    });

    let replicate_url = format!(
        "https://api.replicate.com/v1/models/{}/predictions",
        req.model
    );

    // Call Replicate API with synchronous wait (Prefer: wait)
    let resp = state
        .client
        .post(&replicate_url)
        .header("Authorization", format!("Bearer {replicate_token}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "wait")
        .json(&replicate_body)
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return err_response(
                StatusCode::BAD_GATEWAY,
                &format!("Replicate API error: {e}"),
            );
        }
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return err_response(
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            &format!("Replicate error: {body}"),
        );
    }

    // Parse Replicate response to extract output image URL
    let prediction: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return err_response(
                StatusCode::BAD_GATEWAY,
                &format!("Failed to parse Replicate response: {e}"),
            );
        }
    };

    // Check prediction status
    let pred_status = prediction
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if pred_status != "succeeded" {
        let error_msg = prediction
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return err_response(
            StatusCode::BAD_GATEWAY,
            &format!("Replicate prediction failed ({}): {}", pred_status, error_msg),
        );
    }

    // Extract output — can be a string URL or an array of URLs
    let image_url = prediction
        .get("output")
        .and_then(|v| {
            if let Some(url) = v.as_str() {
                Some(url.to_string())
            } else if let Some(arr) = v.as_array() {
                arr.first().and_then(|u| u.as_str()).map(|s| s.to_string())
            } else {
                None
            }
        });

    let image_url = match image_url {
        Some(url) => url,
        None => {
            return err_response(
                StatusCode::BAD_GATEWAY,
                "No output image in Replicate response",
            );
        }
    };

    // Download the image and return it as base64
    let img_resp = state.client.get(&image_url).send().await;
    let img_bytes = match img_resp {
        Ok(r) if r.status().is_success() => r.bytes().await.unwrap_or_default(),
        Ok(r) => {
            return err_response(
                StatusCode::BAD_GATEWAY,
                &format!("Failed to download image: HTTP {}", r.status()),
            );
        }
        Err(e) => {
            return err_response(
                StatusCode::BAD_GATEWAY,
                &format!("Failed to download image: {e}"),
            );
        }
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&img_bytes);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "image_base64": b64,
            "width": req.width,
            "height": req.height,
        })),
    )
        .into_response()
}

// ── Helpers ──────────────────────────────────────────────────

/// Authenticate request via Bearer token, return user_id or error response.
/// Public so it can be used from main.rs for proxy authentication.
pub fn authenticate(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Uuid, axum::response::Response> {
    let token = extract_bearer_token(headers)
        .ok_or_else(|| err_response(StatusCode::UNAUTHORIZED, "Missing authorization header"))?;

    let claims = auth::verify_token(token, &state.jwt_secret)
        .map_err(|_| err_response(StatusCode::UNAUTHORIZED, "Invalid token"))?;

    Ok(claims.sub)
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let value = headers.get("authorization")?.to_str().ok()?;
    if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
        Some(&value[7..])
    } else {
        None
    }
}

fn err_response(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}
