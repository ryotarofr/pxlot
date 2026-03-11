use crate::{Canvas, Color, PixelBuffer};
use serde::{Deserialize, Serialize};

/// Dithering method for color reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DitherMethod {
    None,
    FloydSteinberg,
    Ordered2x2,
    Ordered4x4,
}

/// Downsampling method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownsampleMethod {
    NearestNeighbor,
    Average,
}

/// Parameters for pixelization processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelizeParams {
    pub target_width: u32,
    pub target_height: u32,
    pub max_colors: usize,
    pub dither: DitherMethod,
    pub downsample: DownsampleMethod,
    /// Optional palette constraint (HEX colors). If provided, map to these colors.
    pub palette: Option<Vec<String>>,
}

impl Default for PixelizeParams {
    fn default() -> Self {
        Self {
            target_width: 32,
            target_height: 32,
            max_colors: 16,
            dither: DitherMethod::None,
            downsample: DownsampleMethod::NearestNeighbor,
            palette: None,
        }
    }
}

/// Downsample a pixel buffer to target dimensions.
pub fn downsample(src: &PixelBuffer, target_w: u32, target_h: u32, method: DownsampleMethod) -> PixelBuffer {
    let mut dst = PixelBuffer::new(target_w, target_h);

    match method {
        DownsampleMethod::NearestNeighbor => {
            for ty in 0..target_h {
                for tx in 0..target_w {
                    let sx = (tx as f64 / target_w as f64 * src.width as f64) as u32;
                    let sy = (ty as f64 / target_h as f64 * src.height as f64) as u32;
                    let sx = sx.min(src.width - 1);
                    let sy = sy.min(src.height - 1);
                    if let Some(&c) = src.get_pixel(sx, sy) {
                        dst.set_pixel(tx, ty, c);
                    }
                }
            }
        }
        DownsampleMethod::Average => {
            let block_w = src.width as f64 / target_w as f64;
            let block_h = src.height as f64 / target_h as f64;

            for ty in 0..target_h {
                for tx in 0..target_w {
                    let sx_start = (tx as f64 * block_w) as u32;
                    let sy_start = (ty as f64 * block_h) as u32;
                    let sx_end = ((tx + 1) as f64 * block_w) as u32;
                    let sy_end = ((ty + 1) as f64 * block_h) as u32;
                    let sx_end = sx_end.min(src.width);
                    let sy_end = sy_end.min(src.height);

                    let mut r_sum = 0u32;
                    let mut g_sum = 0u32;
                    let mut b_sum = 0u32;
                    let mut a_sum = 0u32;
                    let mut count = 0u32;

                    for sy in sy_start..sy_end {
                        for sx in sx_start..sx_end {
                            if let Some(&c) = src.get_pixel(sx, sy) {
                                r_sum += c.r as u32;
                                g_sum += c.g as u32;
                                b_sum += c.b as u32;
                                a_sum += c.a as u32;
                                count += 1;
                            }
                        }
                    }

                    if count > 0 {
                        dst.set_pixel(tx, ty, Color::new(
                            (r_sum / count) as u8,
                            (g_sum / count) as u8,
                            (b_sum / count) as u8,
                            (a_sum / count) as u8,
                        ));
                    }
                }
            }
        }
    }
    dst
}

/// Extract the dominant colors from a pixel buffer using median cut.
pub fn extract_palette(buf: &PixelBuffer, max_colors: usize) -> Vec<Color> {
    let mut colors: Vec<[u8; 3]> = Vec::new();
    for pixel in &buf.pixels {
        if pixel.a > 128 {
            colors.push([pixel.r, pixel.g, pixel.b]);
        }
    }
    if colors.is_empty() {
        return vec![Color::BLACK];
    }

    median_cut(&mut colors, max_colors)
        .into_iter()
        .map(|[r, g, b]| Color::new(r, g, b, 255))
        .collect()
}

/// Simple median cut algorithm for color quantization.
fn median_cut(colors: &mut [[u8; 3]], max: usize) -> Vec<[u8; 3]> {
    if colors.is_empty() {
        return vec![[0, 0, 0]];
    }

    let mut buckets: Vec<Vec<[u8; 3]>> = vec![colors.to_vec()];

    while buckets.len() < max {
        // Find the bucket with the largest range
        let mut best_idx = 0;
        let mut best_range = 0u32;

        for (i, bucket) in buckets.iter().enumerate() {
            if bucket.len() <= 1 {
                continue;
            }
            let range = channel_range(bucket);
            if range > best_range {
                best_range = range;
                best_idx = i;
            }
        }

        if best_range == 0 {
            break;
        }

        let bucket = buckets.remove(best_idx);
        let (a, b) = split_bucket(bucket);
        if !a.is_empty() {
            buckets.push(a);
        }
        if !b.is_empty() {
            buckets.push(b);
        }
    }

    // Average each bucket
    buckets
        .iter()
        .map(|bucket| {
            let (mut r, mut g, mut b) = (0u64, 0u64, 0u64);
            for c in bucket {
                r += c[0] as u64;
                g += c[1] as u64;
                b += c[2] as u64;
            }
            let n = bucket.len() as u64;
            [(r / n) as u8, (g / n) as u8, (b / n) as u8]
        })
        .collect()
}

fn channel_range(colors: &[[u8; 3]]) -> u32 {
    let mut r_range = [255u8, 0u8];
    let mut g_range = [255u8, 0u8];
    let mut b_range = [255u8, 0u8];
    for c in colors {
        r_range[0] = r_range[0].min(c[0]);
        r_range[1] = r_range[1].max(c[0]);
        g_range[0] = g_range[0].min(c[1]);
        g_range[1] = g_range[1].max(c[1]);
        b_range[0] = b_range[0].min(c[2]);
        b_range[1] = b_range[1].max(c[2]);
    }
    let dr = (r_range[1] - r_range[0]) as u32;
    let dg = (g_range[1] - g_range[0]) as u32;
    let db = (b_range[1] - b_range[0]) as u32;
    dr.max(dg).max(db)
}

fn split_bucket(mut colors: Vec<[u8; 3]>) -> (Vec<[u8; 3]>, Vec<[u8; 3]>) {
    // Find which channel has the widest range
    let mut r_range = [255u8, 0u8];
    let mut g_range = [255u8, 0u8];
    let mut b_range = [255u8, 0u8];
    for c in &colors {
        r_range[0] = r_range[0].min(c[0]);
        r_range[1] = r_range[1].max(c[0]);
        g_range[0] = g_range[0].min(c[1]);
        g_range[1] = g_range[1].max(c[1]);
        b_range[0] = b_range[0].min(c[2]);
        b_range[1] = b_range[1].max(c[2]);
    }
    let dr = r_range[1] - r_range[0];
    let dg = g_range[1] - g_range[0];
    let db = b_range[1] - b_range[0];

    let channel = if dr >= dg && dr >= db {
        0
    } else if dg >= db {
        1
    } else {
        2
    };

    colors.sort_by_key(|c| c[channel]);
    let mid = colors.len() / 2;
    let b = colors.split_off(mid);
    (colors, b)
}

/// Reduce colors in a pixel buffer to the given palette.
pub fn reduce_colors(buf: &mut PixelBuffer, palette: &[Color], dither: DitherMethod) {
    match dither {
        DitherMethod::None => reduce_no_dither(buf, palette),
        DitherMethod::FloydSteinberg => reduce_floyd_steinberg(buf, palette),
        DitherMethod::Ordered2x2 => reduce_ordered(buf, palette, 2),
        DitherMethod::Ordered4x4 => reduce_ordered(buf, palette, 4),
    }
}

fn nearest_palette_color(color: Color, palette: &[Color]) -> Color {
    let mut best = palette[0];
    let mut best_dist = u32::MAX;
    for &pc in palette {
        let dr = color.r as i32 - pc.r as i32;
        let dg = color.g as i32 - pc.g as i32;
        let db = color.b as i32 - pc.b as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = pc;
        }
    }
    best
}

fn reduce_no_dither(buf: &mut PixelBuffer, palette: &[Color]) {
    for pixel in &mut buf.pixels {
        if pixel.a < 128 {
            *pixel = Color::TRANSPARENT;
        } else {
            *pixel = nearest_palette_color(*pixel, palette);
        }
    }
}

fn reduce_floyd_steinberg(buf: &mut PixelBuffer, palette: &[Color]) {
    let w = buf.width as usize;
    let h = buf.height as usize;

    // Work with float errors
    let mut errors: Vec<[f32; 3]> = vec![[0.0; 3]; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let pixel = buf.pixels[idx];
            if pixel.a < 128 {
                buf.pixels[idx] = Color::TRANSPARENT;
                continue;
            }

            let r = (pixel.r as f32 + errors[idx][0]).clamp(0.0, 255.0);
            let g = (pixel.g as f32 + errors[idx][1]).clamp(0.0, 255.0);
            let b = (pixel.b as f32 + errors[idx][2]).clamp(0.0, 255.0);

            let adjusted = Color::new(r as u8, g as u8, b as u8, 255);
            let nearest = nearest_palette_color(adjusted, palette);
            buf.pixels[idx] = nearest;

            let er = r - nearest.r as f32;
            let eg = g - nearest.g as f32;
            let eb = b - nearest.b as f32;

            // Distribute error
            if x + 1 < w {
                errors[idx + 1][0] += er * 7.0 / 16.0;
                errors[idx + 1][1] += eg * 7.0 / 16.0;
                errors[idx + 1][2] += eb * 7.0 / 16.0;
            }
            if y + 1 < h {
                if x > 0 {
                    errors[idx + w - 1][0] += er * 3.0 / 16.0;
                    errors[idx + w - 1][1] += eg * 3.0 / 16.0;
                    errors[idx + w - 1][2] += eb * 3.0 / 16.0;
                }
                errors[idx + w][0] += er * 5.0 / 16.0;
                errors[idx + w][1] += eg * 5.0 / 16.0;
                errors[idx + w][2] += eb * 5.0 / 16.0;
                if x + 1 < w {
                    errors[idx + w + 1][0] += er * 1.0 / 16.0;
                    errors[idx + w + 1][1] += eg * 1.0 / 16.0;
                    errors[idx + w + 1][2] += eb * 1.0 / 16.0;
                }
            }
        }
    }
}

fn reduce_ordered(buf: &mut PixelBuffer, palette: &[Color], size: u32) {
    let matrix: &[&[f32]] = if size == 2 {
        &[&[0.0, 2.0], &[3.0, 1.0]]
    } else {
        &[
            &[0.0, 8.0, 2.0, 10.0],
            &[12.0, 4.0, 14.0, 6.0],
            &[3.0, 11.0, 1.0, 9.0],
            &[15.0, 7.0, 13.0, 5.0],
        ]
    };
    let n = (size * size) as f32;

    for y in 0..buf.height {
        for x in 0..buf.width {
            let idx = (y * buf.width + x) as usize;
            let pixel = buf.pixels[idx];
            if pixel.a < 128 {
                buf.pixels[idx] = Color::TRANSPARENT;
                continue;
            }

            let threshold = matrix[(y % size) as usize][(x % size) as usize] / n - 0.5;
            let factor = 32.0; // dither strength

            let r = (pixel.r as f32 + threshold * factor).clamp(0.0, 255.0);
            let g = (pixel.g as f32 + threshold * factor).clamp(0.0, 255.0);
            let b = (pixel.b as f32 + threshold * factor).clamp(0.0, 255.0);

            let adjusted = Color::new(r as u8, g as u8, b as u8, 255);
            buf.pixels[idx] = nearest_palette_color(adjusted, palette);
        }
    }
}

/// Full pixelization pipeline: downsample → extract/constrain palette → reduce colors.
pub fn pixelize(src: &PixelBuffer, params: &PixelizeParams) -> (PixelBuffer, Vec<Color>) {
    // Step 1: Downsample
    let mut result = downsample(src, params.target_width, params.target_height, params.downsample);

    // Step 2: Determine palette
    let palette = if let Some(ref hex_colors) = params.palette {
        hex_colors
            .iter()
            .filter_map(|h| Color::from_hex(h))
            .collect()
    } else {
        extract_palette(&result, params.max_colors)
    };

    // Step 3: Reduce colors
    if !palette.is_empty() {
        reduce_colors(&mut result, &palette, params.dither);
    }

    (result, palette)
}

/// Convert a PixelBuffer into a Canvas (single layer).
/// The buffer pixels are copied into the frame area of the new canvas.
pub fn buffer_to_canvas(buf: PixelBuffer) -> Canvas {
    let w = buf.width;
    let h = buf.height;
    let mut canvas = Canvas::new(w, h);
    let fx = canvas.frame_x;
    let fy = canvas.frame_y;
    for y in 0..h {
        for x in 0..w {
            if let Some(&color) = buf.get_pixel(x, y) {
                canvas.layers[0].buffer.set_pixel(fx + x, fy + y, color);
            }
        }
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gradient_buffer(w: u32, h: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let r = (x * 255 / w.max(1)) as u8;
                let g = (y * 255 / h.max(1)) as u8;
                buf.set_pixel(x, y, Color::new(r, g, 128, 255));
            }
        }
        buf
    }

    #[test]
    fn test_downsample_nearest() {
        let src = make_gradient_buffer(64, 64);
        let dst = downsample(&src, 16, 16, DownsampleMethod::NearestNeighbor);
        assert_eq!(dst.width, 16);
        assert_eq!(dst.height, 16);
        // Top-left should still be dark
        let tl = dst.get_pixel(0, 0).unwrap();
        assert!(tl.r < 20);
        assert!(tl.g < 20);
    }

    #[test]
    fn test_downsample_average() {
        let src = make_gradient_buffer(64, 64);
        let dst = downsample(&src, 8, 8, DownsampleMethod::Average);
        assert_eq!(dst.width, 8);
        assert_eq!(dst.height, 8);
    }

    #[test]
    fn test_extract_palette() {
        let mut buf = PixelBuffer::new(4, 4);
        for i in 0..8 {
            buf.set_pixel(i % 4, i / 4, Color::new(255, 0, 0, 255));
        }
        for i in 8..16 {
            buf.set_pixel(i % 4, i / 4, Color::new(0, 0, 255, 255));
        }
        let palette = extract_palette(&buf, 2);
        assert_eq!(palette.len(), 2);
    }

    #[test]
    fn test_reduce_no_dither() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, Color::new(200, 10, 10, 255));
        buf.set_pixel(1, 0, Color::new(10, 10, 200, 255));
        buf.set_pixel(0, 1, Color::new(180, 20, 30, 255));
        buf.set_pixel(1, 1, Color::new(20, 30, 180, 255));

        let palette = vec![
            Color::new(255, 0, 0, 255),
            Color::new(0, 0, 255, 255),
        ];
        reduce_colors(&mut buf, &palette, DitherMethod::None);

        assert_eq!(buf.get_pixel(0, 0).unwrap().r, 255); // red
        assert_eq!(buf.get_pixel(1, 0).unwrap().b, 255); // blue
    }

    #[test]
    fn test_floyd_steinberg() {
        let mut buf = make_gradient_buffer(8, 8);
        let palette = vec![
            Color::new(0, 0, 0, 255),
            Color::new(128, 128, 128, 255),
            Color::new(255, 255, 255, 255),
        ];
        reduce_colors(&mut buf, &palette, DitherMethod::FloydSteinberg);
        // Just verify no panic and pixels are from palette
        for p in &buf.pixels {
            if p.a > 0 {
                assert!(palette.contains(p), "pixel {:?} not in palette", p);
            }
        }
    }

    #[test]
    fn test_pixelize_pipeline() {
        let src = make_gradient_buffer(64, 64);
        let params = PixelizeParams {
            target_width: 16,
            target_height: 16,
            max_colors: 4,
            dither: DitherMethod::None,
            ..Default::default()
        };
        let (result, palette) = pixelize(&src, &params);
        assert_eq!(result.width, 16);
        assert_eq!(result.height, 16);
        assert!(palette.len() <= 4);
    }
}
