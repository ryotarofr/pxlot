use pxlot_animation::Timeline;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const DB_NAME: &str = "pxlot_projects";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "projects";
/// Max per-project size in bytes (50 MB).
const MAX_PROJECT_SIZE: usize = 50 * 1024 * 1024;
/// Warning threshold (40 MB).
const WARN_PROJECT_SIZE: usize = 40 * 1024 * 1024;

/// Project metadata stored alongside timeline data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub created_at: f64,
    pub updated_at: f64,
}

/// Full project data.
#[derive(Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub meta: ProjectMeta,
    pub timeline: Timeline,
}

/// Size warning level.
pub enum SizeWarning {
    Ok,
    Warn(usize),
    Exceeded(usize),
}

pub fn check_size(json_len: usize) -> SizeWarning {
    if json_len > MAX_PROJECT_SIZE {
        SizeWarning::Exceeded(json_len)
    } else if json_len > WARN_PROJECT_SIZE {
        SizeWarning::Warn(json_len)
    } else {
        SizeWarning::Ok
    }
}

/// Serialize project data to JSON. Returns None on error.
pub fn serialize(data: &ProjectData) -> Option<String> {
    match serde_json::to_string(data) {
        Ok(j) => Some(j),
        Err(e) => {
            log::error!("Failed to serialize project: {}", e);
            None
        }
    }
}

/// Helper: check if db has our object store, create if not.
fn ensure_store(db: &web_sys::IdbDatabase) {
    let names = db.object_store_names();
    let mut has_store = false;
    for i in 0..names.length() {
        if names.get(i).as_deref() == Some(STORE_NAME) {
            has_store = true;
            break;
        }
    }
    if !has_store {
        let params = web_sys::IdbObjectStoreParameters::new();
        params.set_key_path(&JsValue::from_str("meta.name"));
        db.create_object_store_with_optional_parameters(STORE_NAME, &params)
            .unwrap();
    }
}

fn make_upgrade_cb() -> Closure<dyn FnMut(web_sys::Event)> {
    Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let target = ev.target().unwrap();
        let req: web_sys::IdbOpenDbRequest = target.unchecked_into();
        let db: web_sys::IdbDatabase = req.result().unwrap().unchecked_into();
        ensure_store(&db);
    }) as Box<dyn FnMut(_)>)
}

/// Save project to IndexedDB using pre-serialized JSON.
pub fn save_project(name: String, json: String) {
    let window = web_sys::window().unwrap();
    let idb = window.indexed_db().unwrap().unwrap();
    let open_req = idb.open_with_u32(DB_NAME, DB_VERSION).unwrap();

    let on_upgrade = make_upgrade_cb();

    let on_success = Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let target = ev.target().unwrap();
        let req: web_sys::IdbOpenDbRequest = target.unchecked_into();
        let db: web_sys::IdbDatabase = req.result().unwrap().unchecked_into();

        let tx = db
            .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)
            .unwrap();
        let store = tx.object_store(STORE_NAME).unwrap();

        let js_val: JsValue = js_sys::JSON::parse(&json).unwrap();
        let _ = store.put(&js_val);

        log::info!("Project '{}' saved to IndexedDB", name);
    }) as Box<dyn FnMut(_)>);

    open_req.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    open_req.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    on_upgrade.forget();
    on_success.forget();
}

/// Load project from IndexedDB by name. Calls callback with result.
pub fn load_project(name: String, callback: impl FnOnce(Option<ProjectData>) + 'static) {
    let window = web_sys::window().unwrap();
    let idb = window.indexed_db().unwrap().unwrap();
    let open_req = idb.open_with_u32(DB_NAME, DB_VERSION).unwrap();

    let on_upgrade = make_upgrade_cb();
    let callback = std::cell::RefCell::new(Some(callback));

    let on_success = Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let target = ev.target().unwrap();
        let req: web_sys::IdbOpenDbRequest = target.unchecked_into();
        let db: web_sys::IdbDatabase = req.result().unwrap().unchecked_into();

        let tx = db
            .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readonly)
            .unwrap();
        let store = tx.object_store(STORE_NAME).unwrap();
        let get_req = store.get(&JsValue::from_str(&name)).unwrap();

        let cb = std::cell::RefCell::new(callback.borrow_mut().take());
        let on_get = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            let target = ev.target().unwrap();
            let req: web_sys::IdbRequest = target.unchecked_into();
            let result = req.result().unwrap();

            let project = if result.is_undefined() || result.is_null() {
                None
            } else {
                let json = js_sys::JSON::stringify(&result)
                    .map(|s| s.as_string().unwrap_or_default())
                    .unwrap_or_default();
                serde_json::from_str::<ProjectData>(&json).ok()
            };

            if let Some(cb) = cb.borrow_mut().take() {
                cb(project);
            }
        }) as Box<dyn FnMut(_)>);

        get_req.set_onsuccess(Some(on_get.as_ref().unchecked_ref()));
        on_get.forget();
    }) as Box<dyn FnMut(_)>);

    open_req.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    open_req.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    on_upgrade.forget();
    on_success.forget();
}

/// Delete a project from IndexedDB.
pub fn delete_project(name: &str) {
    let window = web_sys::window().unwrap();
    let idb = window.indexed_db().unwrap().unwrap();
    let open_req = idb.open_with_u32(DB_NAME, DB_VERSION).unwrap();
    let name = name.to_string();

    let on_upgrade = make_upgrade_cb();

    let on_success = Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let target = ev.target().unwrap();
        let req: web_sys::IdbOpenDbRequest = target.unchecked_into();
        let db: web_sys::IdbDatabase = req.result().unwrap().unchecked_into();

        let tx = db
            .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)
            .unwrap();
        let store = tx.object_store(STORE_NAME).unwrap();
        let _ = store.delete(&JsValue::from_str(&name));
        log::info!("Project '{}' deleted from IndexedDB", name);
    }) as Box<dyn FnMut(_)>);

    open_req.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    open_req.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    on_upgrade.forget();
    on_success.forget();
}
