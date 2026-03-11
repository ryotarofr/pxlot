use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Ja => "JP",
        }
    }
}

/// Translation keys → values.
pub fn t(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        // App title
        (Lang::En, "app_title") => "PixelForge",
        (Lang::Ja, "app_title") => "PixelForge",

        // Menu
        (Lang::En, "export_png") => "PNG",
        (Lang::Ja, "export_png") => "PNG",
        (Lang::En, "export_gif") => "GIF",
        (Lang::Ja, "export_gif") => "GIF",
        (Lang::En, "import") => "Import",
        (Lang::Ja, "import") => "インポート",
        (Lang::En, "save_project") => "Save",
        (Lang::Ja, "save_project") => "保存",
        (Lang::En, "load_project") => "Load",
        (Lang::Ja, "load_project") => "読込",
        (Lang::En, "undo") => "Undo",
        (Lang::Ja, "undo") => "元に戻す",
        (Lang::En, "redo") => "Redo",
        (Lang::Ja, "redo") => "やり直し",
        (Lang::En, "grid") => "Grid",
        (Lang::Ja, "grid") => "グリッド",

        // Tools
        (Lang::En, "pencil") => "Pencil (B)",
        (Lang::Ja, "pencil") => "鉛筆 (B)",
        (Lang::En, "eraser") => "Eraser (E)",
        (Lang::Ja, "eraser") => "消しゴム (E)",
        (Lang::En, "fill") => "Fill (G)",
        (Lang::Ja, "fill") => "塗りつぶし (G)",
        (Lang::En, "eyedropper") => "Eyedropper (I)",
        (Lang::Ja, "eyedropper") => "スポイト (I)",
        (Lang::En, "line") => "Line (L)",
        (Lang::Ja, "line") => "直線 (L)",
        (Lang::En, "rectangle") => "Rectangle (R)",
        (Lang::Ja, "rectangle") => "矩形 (R)",
        (Lang::En, "ellipse") => "Ellipse (O)",
        (Lang::Ja, "ellipse") => "楕円 (O)",
        (Lang::En, "rect_select") => "Rect Select (M)",
        (Lang::Ja, "rect_select") => "矩形選択 (M)",

        // Layers
        (Lang::En, "layers") => "Layers",
        (Lang::Ja, "layers") => "レイヤー",
        (Lang::En, "add_layer") => "Add Layer",
        (Lang::Ja, "add_layer") => "レイヤー追加",

        // Colors
        (Lang::En, "colors") => "Color",
        (Lang::Ja, "colors") => "カラー",

        // AI panel
        (Lang::En, "ai_assistant") => "AI Assistant",
        (Lang::Ja, "ai_assistant") => "AIアシスタント",
        (Lang::En, "ai_pixelate") => "Pixelate",
        (Lang::Ja, "ai_pixelate") => "ピクセル化",
        (Lang::En, "ai_palette") => "Palette",
        (Lang::Ja, "ai_palette") => "パレット",
        (Lang::En, "ai_execute") => "Execute",
        (Lang::Ja, "ai_execute") => "実行",
        (Lang::En, "ai_processing") => "Processing...",
        (Lang::Ja, "ai_processing") => "処理中...",
        (Lang::En, "ai_offline") => "Offline - AI features unavailable. Local processing still works.",
        (Lang::Ja, "ai_offline") => "オフライン - AI機能は使用できません。ローカル処理は利用可能です。",
        (Lang::En, "ai_apply_palette") => "Apply Palette",
        (Lang::Ja, "ai_apply_palette") => "パレットを適用",
        (Lang::En, "ai_size") => "Size:",
        (Lang::Ja, "ai_size") => "サイズ:",
        (Lang::En, "ai_colors") => "Colors:",
        (Lang::Ja, "ai_colors") => "色数:",
        (Lang::En, "ai_dither") => "Dither:",
        (Lang::Ja, "ai_dither") => "ディザ:",

        // Timeline
        (Lang::En, "add_frame") => "Add",
        (Lang::Ja, "add_frame") => "追加",
        (Lang::En, "duplicate_frame") => "Dup",
        (Lang::Ja, "duplicate_frame") => "複製",
        (Lang::En, "delete_frame") => "Del",
        (Lang::Ja, "delete_frame") => "削除",

        // Onion skin
        (Lang::En, "onion_skin") => "Onion Skin",
        (Lang::Ja, "onion_skin") => "オニオンスキン",

        // Status
        (Lang::En, "canvas_size") => "Canvas",
        (Lang::Ja, "canvas_size") => "キャンバス",
        (Lang::En, "zoom") => "Zoom",
        (Lang::Ja, "zoom") => "ズーム",
        (Lang::En, "history") => "History",
        (Lang::Ja, "history") => "履歴",
        (Lang::En, "frame") => "Frame",
        (Lang::Ja, "frame") => "フレーム",

        // Fallback
        _ => "?"
    }
}

/// Provide i18n context to the app.
pub fn provide_i18n() -> (ReadSignal<Lang>, WriteSignal<Lang>) {
    // Detect browser language
    let initial = detect_lang();
    let (lang, set_lang) = signal(initial);
    (lang, set_lang)
}

fn detect_lang() -> Lang {
    let nav_lang = web_sys::window()
        .and_then(|w| w.navigator().language())
        .unwrap_or_default();
    if nav_lang.starts_with("ja") {
        Lang::Ja
    } else {
        Lang::En
    }
}
