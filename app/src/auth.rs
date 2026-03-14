use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const TOKEN_KEY: &str = "pxlot_auth_token";
const USER_KEY: &str = "pxlot_auth_user";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: AuthUser,
}

/// Save auth token and user to localStorage.
pub fn save_auth(token: &str, user: &AuthUser) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(TOKEN_KEY, token);
        if let Ok(json) = serde_json::to_string(user) {
            let _ = storage.set_item(USER_KEY, &json);
        }
    }
}

/// Load saved auth token.
pub fn load_token() -> Option<String> {
    local_storage()?.get_item(TOKEN_KEY).ok().flatten()
}

/// Load saved user info.
pub fn load_user() -> Option<AuthUser> {
    let json = local_storage()?.get_item(USER_KEY).ok().flatten()?;
    serde_json::from_str(&json).ok()
}

/// Clear auth data (logout).
pub fn clear_auth() {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(TOKEN_KEY);
        let _ = storage.remove_item(USER_KEY);
    }
}

/// Fetch public config (Google Client ID) from the server.
pub async fn fetch_config() -> Result<String, String> {
    let window = web_sys::window().ok_or("No window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let request = web_sys::Request::new_with_str_and_init("/api/config", &opts)
        .map_err(|e| format!("{e:?}"))?;

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;

    let resp: web_sys::Response = resp.dyn_into().map_err(|e| format!("{e:?}"))?;

    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|e| format!("{e:?}"))?,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    let text = text.as_string().ok_or("Response not a string")?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{e}"))?;

    json.get("google_client_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("Missing google_client_id in config".to_string())
}

/// Send Google ID token to our backend and get JWT + user.
pub async fn login_with_google(id_token: &str) -> Result<AuthResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    let body = serde_json::json!({ "id_token": id_token }).to_string();

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body));

    let request = web_sys::Request::new_with_str_and_init("/api/auth/google", &opts)
        .map_err(|e| format!("{e:?}"))?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(|e| format!("{e:?}"))?;

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;

    let resp: web_sys::Response = resp.dyn_into().map_err(|e| format!("{e:?}"))?;

    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|e| format!("{e:?}"))?,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    let text = text.as_string().ok_or("Response not a string")?;

    if !resp.ok() {
        return Err(format!("Auth failed: {text}"));
    }

    let auth_resp: AuthResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;

    save_auth(&auth_resp.token, &auth_resp.user);

    Ok(auth_resp)
}

/// Verify existing token with backend.
pub async fn verify_token() -> Result<AuthUser, String> {
    let token = load_token().ok_or("No token")?;

    let window = web_sys::window().ok_or("No window")?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let request = web_sys::Request::new_with_str_and_init("/api/auth/me", &opts)
        .map_err(|e| format!("{e:?}"))?;
    request
        .headers()
        .set("Authorization", &format!("Bearer {token}"))
        .map_err(|e| format!("{e:?}"))?;

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;

    let resp: web_sys::Response = resp.dyn_into().map_err(|e| format!("{e:?}"))?;

    if !resp.ok() {
        clear_auth();
        return Err("Token expired".to_string());
    }

    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|e| format!("{e:?}"))?,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    let text = text.as_string().ok_or("Response not a string")?;
    let user: AuthUser = serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;

    Ok(user)
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
