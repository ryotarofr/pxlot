mod auth;
mod db;
mod routes;

use axum::{Router, extract::{DefaultBodyLimit, State}, http::StatusCode, response::IntoResponse, routing::{get, post}};
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AppState {
    api_key: String,
    client: Client,
    pool: PgPool,
    jwt_secret: String,
    google_client_id: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set");

    // Connect to PostgreSQL
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    run_migrations(&pool).await;

    let state = Arc::new(AppState {
        api_key,
        client: Client::new(),
        pool,
        jwt_secret,
        google_client_id,
    });

    let allowed_origin = std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let cors = CorsLayer::new()
        .allow_origin(allowed_origin.parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // AI proxy
        .route("/api/messages", post(proxy_messages))
        // Auth
        .route("/api/auth/google", post(routes::google_auth))
        .route("/api/auth/me", get(routes::me))
        .route("/api/config", get(routes::get_config))
        // Gallery (public, no auth)
        .route("/api/gallery", get(routes::list_gallery))
        .route("/api/gallery/{id}", get(routes::get_gallery_detail))
        // Projects
        .route("/api/projects", get(routes::list_projects).post(routes::create_project))
        .route("/api/projects/{id}", get(routes::get_project).put(routes::update_project).delete(routes::delete_project))
        .layer(cors)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind to port 3000");

    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("Server error");
}

/// Run SQL migration files from the migrations/ directory.
async fn run_migrations(pool: &PgPool) {
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");

    if !migrations_dir.exists() {
        println!("No migrations directory found, skipping.");
        return;
    }

    // Create migrations tracking table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name VARCHAR(255) PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"
    )
    .execute(pool)
    .await
    .expect("Failed to create migrations table");

    // Read and sort migration files
    let mut entries: Vec<_> = std::fs::read_dir(&migrations_dir)
        .expect("Failed to read migrations directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();

        // Check if already applied
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)"
        )
        .bind(&name)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if applied {
            continue;
        }

        let sql = std::fs::read_to_string(entry.path())
            .unwrap_or_else(|_| panic!("Failed to read migration {name}"));

        println!("Applying migration: {name}");
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("Migration {name} failed: {e}"));

        sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
            .bind(&name)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("Failed to record migration {name}: {e}"));
    }
}

async fn proxy_messages(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Authenticate
    let _user_id = match routes::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let resp = state
        .client
        .post(ANTHROPIC_API_URL)
        .header("Content-Type", "application/json")
        .header("x-api-key", &state.api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .body(body)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = r.text().await.unwrap_or_default();
            (status, body).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            format!("Proxy error: {e}"),
        )
            .into_response(),
    }
}
