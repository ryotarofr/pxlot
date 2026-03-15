use pxlot_core::{Canvas, Color};

/// Maximum canvas area for SVG export (256x256 = 65536 pixels).
const MAX_SVG_PIXELS: u32 = 256 * 256;

/// Export the flattened canvas to an SVG string.
///
/// Each opaque pixel becomes a `<rect>` element at its (x, y) position.
/// Transparent pixels (alpha == 0) are skipped.
/// Returns an error if the canvas exceeds 256x256 pixels.
pub fn export_svg(canvas: &Canvas) -> Result<String, String> {
    let w = canvas.frame_width();
    let h = canvas.frame_height();

    if w * h > MAX_SVG_PIXELS {
        return Err(format!(
            "Canvas too large for SVG export ({}x{} = {} pixels, max {}). Reduce canvas size first.",
            w,
            h,
            w * h,
            MAX_SVG_PIXELS
        ));
    }

    let flat = canvas.flatten_frame_visible();

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n",
        w, h
    ));

    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let r = flat[i];
            let g = flat[i + 1];
            let b = flat[i + 2];
            let a = flat[i + 3];

            if a == 0 {
                continue;
            }

            let hex = Color::new(r, g, b, a).to_hex();
            if a == 255 {
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"{}\"/>\n",
                    x, y, hex
                ));
            } else {
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"{}\" fill-opacity=\"{:.3}\"/>\n",
                    x, y, hex, a as f64 / 255.0
                ));
            }
        }
    }

    svg.push_str("</svg>\n");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pxlot_core::{Canvas, Color};

    #[test]
    fn test_empty_canvas() {
        let canvas = Canvas::new(4, 4);
        let svg = export_svg(&canvas).unwrap();
        assert!(svg.contains("viewBox=\"0 0 4 4\""));
        // No rect elements for a fully transparent canvas.
        assert!(!svg.contains("<rect"));
    }

    #[test]
    fn test_single_opaque_pixel() {
        let mut canvas = Canvas::new(3, 3);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(fx + 1, fy + 2, Color::new(255, 0, 0, 255));
        let svg = export_svg(&canvas).unwrap();
        assert!(svg.contains("viewBox=\"0 0 3 3\""));
        assert!(svg.contains("<rect x=\"1\" y=\"2\" width=\"1\" height=\"1\" fill=\"#ff0000\"/>"));
        // Only one rect should be present.
        assert_eq!(svg.matches("<rect").count(), 1);
    }

    #[test]
    fn test_semi_transparent_pixel() {
        let mut canvas = Canvas::new(2, 2);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(fx, fy, Color::new(0, 128, 255, 128));
        let svg = export_svg(&canvas).unwrap();
        assert!(svg.contains("fill-opacity="));
        assert!(svg.contains("fill=\"#0080ff\""));
    }

    #[test]
    fn test_transparent_pixels_skipped() {
        let mut canvas = Canvas::new(2, 2);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(fx, fy, Color::TRANSPARENT);
        canvas.layers[0]
            .buffer
            .set_pixel(fx + 1, fy + 1, Color::new(0, 255, 0, 255));
        let svg = export_svg(&canvas).unwrap();
        assert_eq!(svg.matches("<rect").count(), 1);
        assert!(svg.contains("x=\"1\" y=\"1\""));
    }

    #[test]
    fn test_multiple_pixels() {
        let mut canvas = Canvas::new(2, 2);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        canvas.layers[0]
            .buffer
            .set_pixel(fx, fy, Color::new(255, 0, 0, 255));
        canvas.layers[0]
            .buffer
            .set_pixel(fx + 1, fy + 1, Color::new(0, 0, 255, 255));
        let svg = export_svg(&canvas).unwrap();
        assert_eq!(svg.matches("<rect").count(), 2);
    }

    #[test]
    fn test_svg_structure() {
        let canvas = Canvas::new(8, 8);
        let svg = export_svg(&canvas).unwrap();
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.trim_end().ends_with("</svg>"));
    }
}
