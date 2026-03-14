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
