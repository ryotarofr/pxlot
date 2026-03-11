use leptos::prelude::*;
use pixelforge_tools::ToolKind;

#[component]
pub fn ToolPanel(
    current_tool: ReadSignal<ToolKind>,
    set_tool: WriteSignal<ToolKind>,
) -> impl IntoView {
    let drawing_tools = vec![
        (ToolKind::Pencil, "P", "Pencil (B)"),
        (ToolKind::Eraser, "E", "Eraser (E)"),
        (ToolKind::Fill, "F", "Fill (G)"),
        (ToolKind::Eyedropper, "I", "Eyedropper (I)"),
    ];

    let select_tools = vec![
        (ToolKind::RectSelect, "S", "Rect Select (M)"),
    ];

    let shape_tools = vec![
        (ToolKind::Line, "L", "Line (L)"),
        (ToolKind::Rectangle, "R", "Rectangle (R)"),
        (ToolKind::Ellipse, "O", "Ellipse (O)"),
        (ToolKind::FilledRectangle, "\u{25a0}", "Filled Rectangle (Shift+R)"),
        (ToolKind::FilledEllipse, "\u{25cf}", "Filled Ellipse (Shift+O)"),
    ];

    let make_buttons = move |tools: Vec<(ToolKind, &'static str, &'static str)>| {
        tools
            .into_iter()
            .map(|(kind, label, title)| {
                let is_active = move || current_tool.get() == kind;
                view! {
                    <button
                        class:tool-btn=true
                        class:active=is_active
                        title=title
                        on:click=move |_| set_tool.set(kind)
                    >
                        {label}
                    </button>
                }
            })
            .collect::<Vec<_>>()
    };

    let drawing_btns = make_buttons(drawing_tools);
    let select_btns = make_buttons(select_tools);
    let shape_btns = make_buttons(shape_tools);

    view! {
        <aside class="tool-panel" role="toolbar" aria-label="Drawing Tools">
            <div class="tool-group">
                {drawing_btns}
                <div class="tool-separator"></div>
                {select_btns}
                <div class="tool-separator"></div>
                {shape_btns}
            </div>
        </aside>
    }
}
