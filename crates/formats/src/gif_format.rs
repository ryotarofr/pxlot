use pixelforge_animation::Timeline;

/// Export a timeline as an animated GIF.
/// Returns the GIF bytes.
pub fn export_gif(timeline: &Timeline) -> Result<Vec<u8>, String> {
    if timeline.frames.is_empty() {
        return Err("No frames to export".to_string());
    }

    let width = timeline.frames[0].width() as u16;
    let height = timeline.frames[0].height() as u16;

    let mut buf = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buf, width, height, &[])
            .map_err(|e| e.to_string())?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;

        for frame_data in &timeline.frames {
            let rgba = frame_data.flatten();
            let delay = (frame_data.delay_ms + 5) / 10; // GIF delay in centiseconds (rounded)

            // Convert RGBA to indexed color using simple quantization
            let (palette, indices) = quantize_rgba(&rgba, width as u32, height as u32);

            let mut gif_frame = gif::Frame::default();
            gif_frame.width = width;
            gif_frame.height = height;
            gif_frame.delay = delay as u16;
            gif_frame.dispose = gif::DisposalMethod::Background;

            // Set palette and buffer
            gif_frame.palette = Some(palette);
            gif_frame.buffer = std::borrow::Cow::Owned(indices);

            // Check if we have transparency
            gif_frame.transparent = Some(0); // Index 0 is transparent

            encoder
                .write_frame(&gif_frame)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(buf)
}

/// Simple color quantization: build a palette from RGBA data.
/// Returns (palette_rgb_flat, indices).
/// Index 0 is reserved for transparent pixels.
fn quantize_rgba(rgba: &[u8], width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let pixel_count = (width * height) as usize;
    let mut palette: Vec<[u8; 3]> = Vec::with_capacity(256);
    let mut indices = Vec::with_capacity(pixel_count);

    // Reserve index 0 for transparency
    palette.push([0, 0, 0]);

    for i in 0..pixel_count {
        let offset = i * 4;
        if offset + 3 >= rgba.len() {
            indices.push(0);
            continue;
        }
        let r = rgba[offset];
        let g = rgba[offset + 1];
        let b = rgba[offset + 2];
        let a = rgba[offset + 3];

        if a < 128 {
            // Transparent
            indices.push(0);
            continue;
        }

        let color = [r, g, b];
        // Find existing color in palette
        if let Some(idx) = palette.iter().position(|c| *c == color) {
            indices.push(idx as u8);
        } else if palette.len() < 256 {
            let idx = palette.len();
            palette.push(color);
            indices.push(idx as u8);
        } else {
            // Palette full — find nearest color
            let idx = find_nearest(&palette, &color);
            indices.push(idx as u8);
        }
    }

    // Flatten palette to RGB bytes
    let palette_flat: Vec<u8> = palette.iter().flat_map(|c| c.iter().copied()).collect();
    (palette_flat, indices)
}

/// Find the nearest color in the palette by Euclidean distance.
fn find_nearest(palette: &[[u8; 3]], target: &[u8; 3]) -> usize {
    let mut best = 0;
    let mut best_dist = u32::MAX;
    for (i, c) in palette.iter().enumerate().skip(1) {
        // skip 0 = transparent
        let dr = c[0] as i32 - target[0] as i32;
        let dg = c[1] as i32 - target[1] as i32;
        let db = c[2] as i32 - target[2] as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

/// Export a single canvas (non-animated) as a GIF.
pub fn export_single_gif(canvas: &pixelforge_core::Canvas) -> Result<Vec<u8>, String> {
    let mut timeline = Timeline::new(canvas.clone());
    timeline.frames[0].delay_ms = 0;
    export_gif(&timeline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixelforge_core::{Canvas, Color};

    #[test]
    fn test_export_gif_single_frame() {
        let mut canvas = Canvas::new(4, 4);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(fx, fy, Color::new(255, 0, 0, 255));
        canvas.layers[0]
            .buffer
            .set_pixel(fx + 1, fy + 1, Color::new(0, 255, 0, 255));

        let timeline = Timeline::new(canvas);
        let gif_data = export_gif(&timeline).unwrap();
        assert!(!gif_data.is_empty());
        // GIF magic number
        assert_eq!(&gif_data[0..3], b"GIF");
    }

    #[test]
    fn test_export_gif_multi_frame() {
        let canvas = Canvas::new(4, 4);
        let mut timeline = Timeline::new(canvas);
        timeline.add_frame(4, 4);
        // Draw something on frame 2 in frame coords
        let fx = timeline.frames[1].canvas.frame_x;
        let fy = timeline.frames[1].canvas.frame_y;
        timeline.frames[1]
            .canvas
            .layers[0]
            .buffer
            .set_pixel(fx + 2, fy + 2, Color::new(0, 0, 255, 255));

        let gif_data = export_gif(&timeline).unwrap();
        assert!(!gif_data.is_empty());
        assert_eq!(&gif_data[0..3], b"GIF");
    }

    #[test]
    fn test_quantize() {
        let rgba = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 0, 0, // transparent
            255, 0, 0, 255, // red again
        ];
        let (palette, indices) = quantize_rgba(&rgba, 2, 2);
        assert_eq!(indices.len(), 4);
        assert_eq!(indices[2], 0); // transparent
        assert_eq!(indices[0], indices[3]); // same red
        assert_ne!(indices[0], indices[1]); // red != green
        assert!(palette.len() >= 9); // at least 3 colors * 3 bytes
    }
}
