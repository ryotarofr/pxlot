use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use reqwest::Client;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

struct AppState {
    api_key: String,
    client: Client,
}

#[tokio::main]
async fn main() {
    // Load .env file from project root (if present)
    dotenvy::dotenv().ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let state = Arc::new(AppState {
        api_key,
        client: Client::new(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/messages", post(proxy_messages))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind to port 3000");

    println!("Proxy server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("Server error");
}

async fn proxy_messages(
    State(state): State<Arc<AppState>>,
    body: String,
) -> impl IntoResponse {
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
