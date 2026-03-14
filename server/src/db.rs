use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub google_id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// Find user by Google ID, or create a new one.
pub async fn upsert_user(
    pool: &PgPool,
    google_id: &str,
    email: &str,
    name: &str,
    avatar_url: Option<&str>,
) -> Result<User, sqlx::Error> {
    let row: User = sqlx::query_as(
        r#"
        INSERT INTO users (google_id, email, name, avatar_url)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (google_id) DO UPDATE
            SET email = EXCLUDED.email,
                name = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
        RETURNING id, google_id, email, name, avatar_url
        "#,
    )
    .bind(google_id)
    .bind(email)
    .bind(name)
    .bind(avatar_url)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Find user by UUID.
pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
    let row: Option<User> =
        sqlx::query_as(r#"SELECT id, google_id, email, name, avatar_url FROM users WHERE id = $1"#)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    Ok(row)
}

// ── Projects ─────────────────────────────────────────────────

/// Project metadata (without data payload).
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct ProjectMeta {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full project with data payload.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub is_public: bool,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Gallery item — public project with author info (list view).
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct GalleryItem {
    pub id: Uuid,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub author_avatar: Option<String>,
}

/// Gallery detail — includes frame thumbnails for frame-by-frame viewing.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct GalleryDetail {
    pub id: Uuid,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub frame_count: i32,
    pub thumbnail: Option<String>,
    pub thumbnail_gif: Option<String>,
    pub frame_thumbnails: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub author_name: String,
    pub author_avatar: Option<String>,
}

/// List all projects for a user (metadata only).
pub async fn list_projects(pool: &PgPool, user_id: Uuid) -> Result<Vec<ProjectMeta>, sqlx::Error> {
    let rows: Vec<ProjectMeta> = sqlx::query_as(
        r#"SELECT id, user_id, name, width, height, frame_count, thumbnail, thumbnail_gif, is_public, created_at, updated_at
           FROM projects WHERE user_id = $1 ORDER BY updated_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get a single project by id (must belong to user).
pub async fn get_project(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Project>, sqlx::Error> {
    let row: Option<Project> = sqlx::query_as(
        r#"SELECT id, user_id, name, width, height, frame_count, thumbnail, thumbnail_gif, is_public, data, created_at, updated_at
           FROM projects WHERE id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Create a new project.
pub async fn create_project(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    width: i32,
    height: i32,
    frame_count: i32,
    thumbnail: Option<&str>,
    thumbnail_gif: Option<&str>,
    is_public: bool,
    frame_thumbnails: Option<&serde_json::Value>,
    data: &serde_json::Value,
) -> Result<ProjectMeta, sqlx::Error> {
    let row: ProjectMeta = sqlx::query_as(
        r#"INSERT INTO projects (user_id, name, width, height, frame_count, thumbnail, thumbnail_gif, is_public, frame_thumbnails, data)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING id, user_id, name, width, height, frame_count, thumbnail, thumbnail_gif, is_public, created_at, updated_at"#,
    )
    .bind(user_id)
    .bind(name)
    .bind(width)
    .bind(height)
    .bind(frame_count)
    .bind(thumbnail)
    .bind(thumbnail_gif)
    .bind(is_public)
    .bind(frame_thumbnails)
    .bind(data)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Update an existing project (must belong to user).
pub async fn update_project(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    name: &str,
    width: i32,
    height: i32,
    frame_count: i32,
    thumbnail: Option<&str>,
    thumbnail_gif: Option<&str>,
    is_public: bool,
    frame_thumbnails: Option<&serde_json::Value>,
    data: &serde_json::Value,
) -> Result<Option<ProjectMeta>, sqlx::Error> {
    let row: Option<ProjectMeta> = sqlx::query_as(
        r#"UPDATE projects
           SET name = $3, width = $4, height = $5, frame_count = $6,
               thumbnail = $7, thumbnail_gif = $8, is_public = $9,
               frame_thumbnails = $10, data = $11, updated_at = NOW()
           WHERE id = $1 AND user_id = $2
           RETURNING id, user_id, name, width, height, frame_count, thumbnail, thumbnail_gif, is_public, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(name)
    .bind(width)
    .bind(height)
    .bind(frame_count)
    .bind(thumbnail)
    .bind(thumbnail_gif)
    .bind(is_public)
    .bind(frame_thumbnails)
    .bind(data)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// List public projects for the gallery (with author info).
pub async fn list_gallery(pool: &PgPool) -> Result<Vec<GalleryItem>, sqlx::Error> {
    let rows: Vec<GalleryItem> = sqlx::query_as(
        r#"SELECT p.id, p.name, p.width, p.height, p.frame_count,
                  p.thumbnail, p.thumbnail_gif, p.created_at,
                  u.name AS author_name, u.avatar_url AS author_avatar
           FROM projects p
           JOIN users u ON u.id = p.user_id
           WHERE p.is_public = TRUE
           ORDER BY p.updated_at DESC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get a single public gallery item with frame thumbnails.
pub async fn get_gallery_detail(pool: &PgPool, project_id: Uuid) -> Result<Option<GalleryDetail>, sqlx::Error> {
    let row: Option<GalleryDetail> = sqlx::query_as(
        r#"SELECT p.id, p.name, p.width, p.height, p.frame_count,
                  p.thumbnail, p.thumbnail_gif, p.frame_thumbnails, p.created_at,
                  u.name AS author_name, u.avatar_url AS author_avatar
           FROM projects p
           JOIN users u ON u.id = p.user_id
           WHERE p.id = $1 AND p.is_public = TRUE"#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Delete a project (must belong to user).
pub async fn delete_project(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM projects WHERE id = $1 AND user_id = $2")
        .bind(project_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
