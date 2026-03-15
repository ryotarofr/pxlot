/// Claude Messages API client for browser-based WASM.
/// Uses web_sys::fetch directly (no external HTTP crate needed).
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

// ── Request types ──────────────────────────────────────────────

#[derive(Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: usize,
    pub system: String,
    pub messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ApiMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

// ── Tool definition ────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ── Response types ─────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct MessagesResponse {
    pub content: Vec<ContentBlock>,
    #[allow(dead_code)]
    pub stop_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

// ── Error response ─────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct ApiError {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub error_type: String,
    pub error: ApiErrorDetail,
}

#[derive(Deserialize, Debug)]
pub struct ApiErrorDetail {
    pub message: String,
}

// ── Client ─────────────────────────────────────────────────────

const API_URL: &str = "/api/messages";

/// Send a Messages API request via the backend proxy and return the parsed response.
pub async fn send_message(request: &MessagesRequest) -> Result<MessagesResponse, String> {
    let body = serde_json::to_string(request).map_err(|e| format!("Serialize error: {e}"))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");

    // Set body
    let body_js = wasm_bindgen::JsValue::from_str(&body);
    opts.set_body(&body_js);

    // Build headers — API key is handled by the backend proxy
    let headers = web_sys::Headers::new().map_err(|e| format!("Headers error: {e:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("Header error: {e:?}"))?;
    // Attach JWT auth token for the backend proxy
    if let Some(token) = crate::auth::load_token() {
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(|e| format!("Auth header error: {e:?}"))?;
    }
    opts.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(API_URL, &opts)
        .map_err(|e| format!("Request error: {e:?}"))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {e:?}"))?;

    let resp: web_sys::Response = resp_val
        .dyn_into()
        .map_err(|_| "Response cast error".to_string())?;

    let text_promise = resp.text().map_err(|e| format!("Text error: {e:?}"))?;
    let text_val = JsFuture::from(text_promise)
        .await
        .map_err(|e| format!("Text read error: {e:?}"))?;
    let text = text_val
        .as_string()
        .ok_or_else(|| "Response not string".to_string())?;

    if !resp.ok() {
        // Try to parse API error
        if let Ok(api_err) = serde_json::from_str::<ApiError>(&text) {
            return Err(format!(
                "API error ({}): {}",
                resp.status(),
                api_err.error.message
            ));
        }
        return Err(format!("HTTP {}: {}", resp.status(), text));
    }

    serde_json::from_str::<MessagesResponse>(&text)
        .map_err(|e| format!("Parse error: {e}\nBody: {text}"))
}
