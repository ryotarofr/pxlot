use pxlot_core::Canvas;
use pxlot_core::history::History;
use std::cell::Cell;

const STORAGE_KEY: &str = "pxlot_autosave";
const HISTORY_KEY: &str = "pxlot_history";
/// Minimum interval between autosaves in milliseconds.
const AUTOSAVE_INTERVAL_MS: f64 = 3000.0;

thread_local! {
    static LAST_AUTOSAVE: Cell<f64> = const { Cell::new(0.0) };
}

/// Save canvas and history state to localStorage (throttled to once per 3 seconds).
pub fn autosave(canvas: &Canvas, history: &History) {
    let now = js_sys::Date::now();
    let should_save = LAST_AUTOSAVE.with(|last| {
        if now - last.get() >= AUTOSAVE_INTERVAL_MS {
            last.set(now);
            true
        } else {
            false
        }
    });
    if !should_save {
        return;
    }

    let Ok(json) = serde_json::to_string(canvas) else {
        log::warn!("Failed to serialize canvas for autosave");
        return;
    };
    let Some(storage) = get_storage() else { return };
    if let Err(e) = storage.set_item(STORAGE_KEY, &json) {
        log::warn!("Autosave failed: {:?}", e);
    }

    // Save history separately (may be larger, separate key avoids breaking canvas restore)
    match serde_json::to_string(history) {
        Ok(hist_json) => {
            if let Err(e) = storage.set_item(HISTORY_KEY, &hist_json) {
                log::warn!("History save failed: {:?}", e);
            }
        }
        Err(e) => {
            log::warn!("Failed to serialize history: {}", e);
        }
    }
}

fn get_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}
