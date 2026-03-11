use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

/// WebGL 2.0 renderer for layer compositing and display.
pub struct WebGlRenderer {
    gl: WebGl2RenderingContext,
    canvas: HtmlCanvasElement,
}

impl WebGlRenderer {
    /// Initialize WebGL 2.0 context from a canvas element.
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let gl = canvas
            .get_context("webgl2")
            .map_err(|e| format!("Failed to get WebGL2 context: {e:?}"))?
            .ok_or("WebGL2 not supported")?
            .dyn_into::<WebGl2RenderingContext>()
            .map_err(|_| "Failed to cast to WebGl2RenderingContext")?;

        gl.clear_color(0.1, 0.1, 0.1, 1.0);
        gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

        log::info!("WebGL 2.0 renderer initialized");

        Ok(Self { gl, canvas })
    }

    /// Clear the canvas with background color.
    pub fn clear(&self) {
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    }

    pub fn canvas(&self) -> &HtmlCanvasElement {
        &self.canvas
    }

    pub fn gl(&self) -> &WebGl2RenderingContext {
        &self.gl
    }
}
