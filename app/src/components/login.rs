use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::auth::{self, AuthUser};

/// Login screen component with Google Sign-In.
#[component]
pub fn LoginScreen(
    /// Callback when login succeeds.
    on_login: Callback<AuthUser>,
    /// Google Client ID for the sign-in button.
    google_client_id: String,
) -> impl IntoView {
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    // Initialize Google Sign-In when component mounts
    let client_id = google_client_id.clone();
    Effect::new(move |_| {
        let client_id = client_id.clone();
        let on_login = on_login.clone();
        let set_error = set_error.clone();
        let set_loading = set_loading.clone();

        // Set up the global callback that Google's SDK will invoke
        let callback = Closure::wrap(Box::new(move |response: JsValue| {
            let credential = js_sys::Reflect::get(&response, &JsValue::from_str("credential"))
                .ok()
                .and_then(|v| v.as_string());

            let Some(id_token) = credential else {
                set_error.set(Some("Google sign-in failed".to_string()));
                return;
            };

            set_loading.set(true);
            let on_login = on_login.clone();
            let set_error = set_error.clone();
            let set_loading = set_loading.clone();

            leptos::task::spawn_local(async move {
                match auth::login_with_google(&id_token).await {
                    Ok(resp) => {
                        on_login.run(resp.user);
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                        set_loading.set(false);
                    }
                }
            });
        }) as Box<dyn FnMut(JsValue)>);

        // Register global callback
        let window = web_sys::window().unwrap();
        let _ = js_sys::Reflect::set(
            &window,
            &JsValue::from_str("handleGoogleCredential"),
            callback.as_ref(),
        );
        callback.forget();

        // Initialize Google Identity Services
        init_google_signin(&client_id);
    });

    view! {
        <div class="login-screen">
            <div class="login-card">
                <h1 class="login-title">"Pxlot"</h1>
                <p class="login-subtitle">"Pixel Art Editor"</p>

                {move || loading.get().then(|| view! {
                    <p class="login-loading">"Signing in..."</p>
                })}

                {move || error.get().map(|e| view! {
                    <p class="login-error">{e}</p>
                })}

                <div id="google-signin-button"></div>

                <p class="login-note">"Sign in with Google to save your work and access AI features."</p>
            </div>
        </div>
    }
}

/// Load and initialize Google Identity Services SDK.
fn init_google_signin(client_id: &str) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    // Check if script already loaded
    if js_sys::Reflect::get(&window, &JsValue::from_str("google"))
        .ok()
        .is_some_and(|v| !v.is_undefined())
    {
        render_google_button(client_id);
        return;
    }

    // Load the Google Identity Services script
    let script = document
        .create_element("script")
        .unwrap();
    script.set_attribute("src", "https://accounts.google.com/gsi/client").unwrap();
    script.set_attribute("async", "").unwrap();

    let client_id = client_id.to_string();
    let onload = Closure::wrap(Box::new(move || {
        render_google_button(&client_id);
    }) as Box<dyn FnMut()>);

    script
        .add_event_listener_with_callback("load", onload.as_ref().unchecked_ref())
        .unwrap();
    onload.forget();

    document.head().unwrap().append_child(&script).unwrap();
}

/// Render the Google Sign-In button after SDK is loaded.
fn render_google_button(client_id: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    // Check if google.accounts exists
    let google = match js_sys::Reflect::get(&window, &JsValue::from_str("google")) {
        Ok(g) if !g.is_undefined() => g,
        _ => return,
    };
    let accounts = match js_sys::Reflect::get(&google, &JsValue::from_str("accounts")) {
        Ok(a) if !a.is_undefined() => a,
        _ => return,
    };
    let id = match js_sys::Reflect::get(&accounts, &JsValue::from_str("id")) {
        Ok(i) if !i.is_undefined() => i,
        _ => return,
    };

    // Call google.accounts.id.initialize({client_id, callback})
    let init_opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&init_opts, &JsValue::from_str("client_id"), &JsValue::from_str(client_id));
    let handle_credential = js_sys::Reflect::get(&window, &JsValue::from_str("handleGoogleCredential")).unwrap_or(JsValue::UNDEFINED);
    let _ = js_sys::Reflect::set(&init_opts, &JsValue::from_str("callback"), &handle_credential);

    if let Ok(initialize) = js_sys::Reflect::get(&id, &JsValue::from_str("initialize")) {
        if let Some(func) = initialize.dyn_ref::<js_sys::Function>() {
            let _ = func.call1(&id, &init_opts);
        }
    }

    // Call google.accounts.id.renderButton(el, {theme, size, width, text})
    let document = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => return,
    };
    if let Some(el) = document.get_element_by_id("google-signin-button") {
        let btn_opts = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&btn_opts, &JsValue::from_str("theme"), &JsValue::from_str("filled_black"));
        let _ = js_sys::Reflect::set(&btn_opts, &JsValue::from_str("size"), &JsValue::from_str("large"));
        let _ = js_sys::Reflect::set(&btn_opts, &JsValue::from_str("width"), &JsValue::from_f64(280.0));
        let _ = js_sys::Reflect::set(&btn_opts, &JsValue::from_str("text"), &JsValue::from_str("signin_with"));

        if let Ok(render_button) = js_sys::Reflect::get(&id, &JsValue::from_str("renderButton")) {
            if let Some(func) = render_button.dyn_ref::<js_sys::Function>() {
                let _ = func.call2(&id, &el, &btn_opts);
            }
        }
    }
}
