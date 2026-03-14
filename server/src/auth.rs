use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims stored in our app tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,  // user id
    pub exp: usize, // expiry (unix timestamp)
}

/// Create a JWT for the given user id.
pub fn create_token(user_id: Uuid, secret: &str) -> Result<String, String> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .ok_or_else(|| "Failed to compute token expiry timestamp".to_string())?
        .timestamp() as usize;

    let claims = Claims { sub: user_id, exp };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Failed to encode JWT: {e}"))
}

/// Verify a JWT and return the claims.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

/// Google ID token payload (subset of fields we need).
#[derive(Debug, Deserialize)]
pub struct GoogleIdTokenPayload {
    /// Google's unique user id
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    /// Audience — should match our Google Client ID
    pub aud: String,
}

/// Verify a Google ID token by calling Google's tokeninfo endpoint.
pub async fn verify_google_id_token(
    client: &reqwest::Client,
    id_token: &str,
    google_client_id: &str,
) -> Result<GoogleIdTokenPayload, String> {
    let url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={id_token}");

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to verify token with Google: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Google token verification failed: {body}"));
    }

    let payload: GoogleIdTokenPayload = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google token response: {e}"))?;

    if payload.aud != google_client_id {
        return Err("Token audience does not match Google Client ID".to_string());
    }

    Ok(payload)
}
