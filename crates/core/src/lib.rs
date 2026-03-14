pub mod dirty_region;
pub mod history;
pub mod image_processing;

use serde::{Deserialize, Serialize};

/// RGBA color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_css(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!(
                "rgba({},{},{},{})",
                self.r,
                self.g,
                self.b,
                self.a as f64 / 255.0
            )
        }
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Self::new(r, g, b, 255))
        } else {
            None
        }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// Pixel buffer backing a single layer.
#[derive(Clone, Serialize, Deserialize)]
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Color>,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let pixels = vec![Color::TRANSPARENT; (width * height) as usize];
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&Color> {
        if x < self.width && y < self.height {
            Some(&self.pixels[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = color;
        }
    }

    /// Returns the raw RGBA bytes for rendering.
    pub fn as_rgba_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for c in &self.pixels {
            bytes.extend_from_slice(&[c.r, c.g, c.b, c.a]);
        }
        bytes
    }

    /// Memory size in bytes.
    pub fn byte_size(&self) -> usize {
        self.pixels.len() * std::mem::size_of::<Color>()
    }
}

/// Blend mode for layer compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
}

/// A single layer in the canvas.
#[derive(Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pub buffer: PixelBuffer,
}

impl Layer {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            buffer: PixelBuffer::new(width, height),
        }
    }
}

/// Alpha-composite a source pixel (with layer opacity) onto a destination RGBA slice.
#[inline]
fn blend_pixel(dst: &mut [u8], src: &Color, layer_opacity: f64) {
    if src.a == 0 {
        return;
    }
    let sa = (src.a as f64 / 255.0) * layer_opacity;
    let dr = dst[0] as f64 / 255.0;
    let dg = dst[1] as f64 / 255.0;
    let db = dst[2] as f64 / 255.0;
    let da = dst[3] as f64 / 255.0;
    let (sr, sg, sb) = (
        src.r as f64 / 255.0,
        src.g as f64 / 255.0,
        src.b as f64 / 255.0,
    );
    let out_a = sa + da * (1.0 - sa);
    if out_a > 0.0 {
        dst[0] = ((sr * sa + dr * da * (1.0 - sa)) / out_a * 255.0 + 0.5) as u8;
        dst[1] = ((sg * sa + dg * da * (1.0 - sa)) / out_a * 255.0 + 0.5) as u8;
        dst[2] = ((sb * sa + db * da * (1.0 - sa)) / out_a * 255.0 + 0.5) as u8;
        dst[3] = (out_a * 255.0 + 0.5) as u8;
    }
}

/// Memory budget constant (256 MB).
const MEMORY_BUDGET: usize = 256 * 1024 * 1024;

/// The main canvas holding all layers.
/// `width`/`height` are the full buffer dimensions.
/// `frame_x`/`frame_y`/`frame_w`/`frame_h` define the export region within the buffer.
#[derive(Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active_layer: usize,
    /// Frame origin X within the buffer.
    #[serde(default)]
    pub frame_x: u32,
    /// Frame origin Y within the buffer.
    #[serde(default)]
    pub frame_y: u32,
    /// Frame width (export width). 0 means use `width` (backward compat).
    #[serde(default)]
    pub frame_w: u32,
    /// Frame height (export height). 0 means use `height` (backward compat).
    #[serde(default)]
    pub frame_h: u32,
}

impl Canvas {
    pub fn new(frame_width: u32, frame_height: u32) -> Self {
        let frame_width = frame_width.max(1);
        let frame_height = frame_height.max(1);
        // Margin for pan/zoom working area, capped to avoid memory explosion on large imports.
        let margin = (frame_width.max(frame_height)).min(64);
        let buf_w = frame_width + 2 * margin;
        let buf_h = frame_height + 2 * margin;
        let initial_layer = Layer::new("Layer 1", buf_w, buf_h);
        Self {
            width: buf_w,
            height: buf_h,
            layers: vec![initial_layer],
            active_layer: 0,
            frame_x: margin,
            frame_y: margin,
            frame_w: frame_width,
            frame_h: frame_height,
        }
    }

    /// Frame width for export. Handles backward compat (frame_w==0).
    pub fn frame_width(&self) -> u32 {
        if self.frame_w > 0 {
            self.frame_w
        } else {
            self.width
        }
    }

    /// Frame height for export. Handles backward compat (frame_h==0).
    pub fn frame_height(&self) -> u32 {
        if self.frame_h > 0 {
            self.frame_h
        } else {
            self.height
        }
    }

    /// Convert frame-relative X to buffer X.
    pub fn to_buf_x(&self, fx: i32) -> i32 {
        fx + self.frame_x as i32
    }

    /// Convert frame-relative Y to buffer Y.
    pub fn to_buf_y(&self, fy: i32) -> i32 {
        fy + self.frame_y as i32
    }

    pub fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        self.layers.get_mut(self.active_layer)
    }

    pub fn active_layer_ref(&self) -> Option<&Layer> {
        self.layers.get(self.active_layer)
    }

    /// Memory usage in bytes for all layers.
    pub fn memory_usage(&self) -> usize {
        self.layers.iter().map(|l| l.buffer.byte_size()).sum()
    }

    /// Check if adding a new layer would exceed the memory budget.
    pub fn can_add_layer(&self) -> bool {
        let layer_size = (self.width * self.height) as usize * std::mem::size_of::<Color>();
        self.memory_usage() + layer_size <= MEMORY_BUDGET
    }

    /// Add a new layer. Returns the index, or None if over budget.
    pub fn add_layer(&mut self, name: impl Into<String>) -> Option<usize> {
        if !self.can_add_layer() {
            return None;
        }
        let layer = Layer::new(name, self.width, self.height);
        self.layers.push(layer);
        let idx = self.layers.len() - 1;
        self.active_layer = idx;
        Some(idx)
    }

    /// Remove a layer by index. Returns the removed layer if valid.
    /// Cannot remove the last remaining layer.
    pub fn remove_layer(&mut self, index: usize) -> Option<Layer> {
        if self.layers.len() <= 1 || index >= self.layers.len() {
            return None;
        }
        let removed = self.layers.remove(index);
        if self.active_layer >= self.layers.len() {
            self.active_layer = self.layers.len() - 1;
        }
        Some(removed)
    }

    /// Move a layer from one position to another.
    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return false;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        // Adjust active_layer
        if self.active_layer == from {
            self.active_layer = to;
        } else if from < self.active_layer && to >= self.active_layer {
            self.active_layer -= 1;
        } else if from > self.active_layer && to <= self.active_layer {
            self.active_layer += 1;
        }
        true
    }

    /// Duplicate the active layer.
    pub fn duplicate_active_layer(&mut self) -> Option<usize> {
        if !self.can_add_layer() {
            return None;
        }
        let layer = self.layers[self.active_layer].clone();
        let mut new_layer = layer;
        new_layer.name = format!("{} copy", self.layers[self.active_layer].name);
        let insert_idx = self.active_layer + 1;
        self.layers.insert(insert_idx, new_layer);
        self.active_layer = insert_idx;
        Some(insert_idx)
    }

    /// Flatten all visible layers into a single PixelBuffer.
    pub fn flatten(&self) -> PixelBuffer {
        let rgba = self.flatten_visible();
        let mut buf = PixelBuffer::new(self.width, self.height);
        for (i, pixel) in buf.pixels.iter_mut().enumerate() {
            let off = i * 4;
            pixel.r = rgba[off];
            pixel.g = rgba[off + 1];
            pixel.b = rgba[off + 2];
            pixel.a = rgba[off + 3];
        }
        buf
    }

    /// Flatten all visible layers into a single RGBA buffer for display.
    pub fn flatten_visible(&self) -> Vec<u8> {
        let size = (self.width * self.height) as usize;
        let mut result = vec![0u8; size * 4]; // RGBA

        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let layer_opacity = layer.opacity as f64 / 255.0;
            for i in 0..size {
                let src = &layer.buffer.pixels[i];
                blend_pixel(&mut result[i * 4..i * 4 + 4], src, layer_opacity);
            }
        }
        result
    }

    /// Flatten visible layers for a sub-region of the buffer into RGBA bytes.
    /// The output is `(region_w * region_h * 4)` bytes, row-major.
    pub fn flatten_region(&self, rx0: u32, ry0: u32, rx1: u32, ry1: u32) -> Vec<u8> {
        let rx0 = rx0.min(self.width);
        let ry0 = ry0.min(self.height);
        let rx1 = rx1.min(self.width);
        let ry1 = ry1.min(self.height);
        let rw = rx1.saturating_sub(rx0) as usize;
        let rh = ry1.saturating_sub(ry0) as usize;
        let size = rw * rh;
        let mut result = vec![0u8; size * 4];

        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let layer_opacity = layer.opacity as f64 / 255.0;
            let bw = self.width as usize;
            for ry in 0..rh {
                let by = ry0 as usize + ry;
                for rx in 0..rw {
                    let bx = rx0 as usize + rx;
                    let buf_i = by * bw + bx;
                    let src = &layer.buffer.pixels[buf_i];
                    let di = (ry * rw + rx) * 4;
                    blend_pixel(&mut result[di..di + 4], src, layer_opacity);
                }
            }
        }
        result
    }

    /// Flatten only the frame region into RGBA bytes (for export).
    pub fn flatten_frame_visible(&self) -> Vec<u8> {
        let fw = self.frame_width();
        let fh = self.frame_height();
        let fx0 = self.frame_x;
        let fy0 = self.frame_y;
        let bw = self.width;
        let bh = self.height;
        let size = (fw * fh) as usize;
        let mut result = vec![0u8; size * 4];

        // Bounds check: ensure frame region fits within buffer
        if fx0 + fw > bw || fy0 + fh > bh {
            return result;
        }

        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let layer_opacity = layer.opacity as f64 / 255.0;
            for fy in 0..fh {
                for fx in 0..fw {
                    let bx = fx + fx0;
                    let by = fy + fy0;
                    let buf_i = (by * bw + bx) as usize;
                    let src = &layer.buffer.pixels[buf_i];
                    let di = (fy * fw + fx) as usize * 4;
                    blend_pixel(&mut result[di..di + 4], src, layer_opacity);
                }
            }
        }
        result
    }

    /// Flatten only the frame region into a PixelBuffer (for export).
    pub fn flatten_frame(&self) -> PixelBuffer {
        let fw = self.frame_width();
        let fh = self.frame_height();
        let rgba = self.flatten_frame_visible();
        let mut buf = PixelBuffer::new(fw, fh);
        for (i, pixel) in buf.pixels.iter_mut().enumerate() {
            let off = i * 4;
            pixel.r = rgba[off];
            pixel.g = rgba[off + 1];
            pixel.b = rgba[off + 2];
            pixel.a = rgba[off + 3];
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_buffer_get_set() {
        let mut buf = PixelBuffer::new(4, 4);
        let red = Color::new(255, 0, 0, 255);
        buf.set_pixel(1, 2, red);
        assert_eq!(buf.get_pixel(1, 2), Some(&red));
        assert_eq!(buf.get_pixel(0, 0), Some(&Color::TRANSPARENT));
    }

    #[test]
    fn test_out_of_bounds() {
        let buf = PixelBuffer::new(4, 4);
        assert_eq!(buf.get_pixel(4, 0), None);
        assert_eq!(buf.get_pixel(0, 4), None);
    }

    #[test]
    fn test_canvas_memory_usage() {
        let canvas = Canvas::new(32, 32);
        // margin = min(32, 64) = 32, buffer = 32 + 2*32 = 96x96
        assert_eq!(canvas.memory_usage(), 96 * 96 * 4);
    }

    #[test]
    fn test_canvas_frame_fields() {
        let canvas = Canvas::new(32, 32);
        assert_eq!(canvas.frame_width(), 32);
        assert_eq!(canvas.frame_height(), 32);
        assert_eq!(canvas.frame_x, 32); // margin = min(32, 64) = 32
        assert_eq!(canvas.frame_y, 32);
        assert_eq!(canvas.width, 96); // 32 + 2*32
        assert_eq!(canvas.height, 96);
    }

    #[test]
    fn test_add_remove_layer() {
        let mut canvas = Canvas::new(8, 8);
        assert_eq!(canvas.layers.len(), 1);
        canvas.add_layer("Layer 2");
        assert_eq!(canvas.layers.len(), 2);
        assert_eq!(canvas.active_layer, 1);
        canvas.remove_layer(0);
        assert_eq!(canvas.layers.len(), 1);
        assert_eq!(canvas.layers[0].name, "Layer 2");
        // Cannot remove the last layer
        assert!(canvas.remove_layer(0).is_none());
    }

    #[test]
    fn test_color_hex() {
        let c = Color::from_hex("#ff8800").unwrap();
        assert_eq!(c, Color::new(255, 136, 0, 255));
        assert_eq!(c.to_hex(), "#ff8800");
    }

    #[test]
    fn test_flatten_frame_visible() {
        let mut canvas = Canvas::new(2, 2);
        // Set pixel at frame (0,0) = buffer (margin, margin)
        let bx = canvas.frame_x;
        let by = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(bx, by, Color::new(255, 0, 0, 255));
        let flat = canvas.flatten_frame_visible();
        assert_eq!(flat[0], 255); // R
        assert_eq!(flat[1], 0); // G
        assert_eq!(flat[2], 0); // B
        assert_eq!(flat[3], 255); // A
    }
}
