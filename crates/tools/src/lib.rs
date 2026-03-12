use pxlot_core::{Canvas, Color};
use pxlot_core::history::Command;
use std::collections::VecDeque;

/// Available drawing tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    RectSelect,
    Line,
    Rectangle,
    Ellipse,
    FilledRectangle,
    FilledEllipse,
}

/// Draw a single pixel, recording the change in the command.
pub fn pencil_pixel(canvas: &mut Canvas, x: u32, y: u32, color: Color, cmd: &mut Command) {
    let layer_idx = canvas.active_layer;
    if let Some(layer) = canvas.active_layer_mut() {
        if layer.locked || !layer.visible {
            return;
        }
        if let Some(&old) = layer.buffer.get_pixel(x, y) {
            if old != color {
                cmd.add_change(layer_idx, x, y, old, color);
                layer.buffer.set_pixel(x, y, color);
            }
        }
    }
}

/// Draw a line of pixels using Bresenham's algorithm (for drag drawing).
pub fn pencil_line(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;

    loop {
        if cx >= 0 && cy >= 0 {
            pencil_pixel(canvas, cx as u32, cy as u32, color, cmd);
        }
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Erase pixels along a line (set to transparent).
pub fn eraser_line(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    cmd: &mut Command,
) {
    pencil_line(canvas, x0, y0, x1, y1, Color::TRANSPARENT, cmd);
}

/// Maximum number of pixels that can be filled in a single flood fill operation.
const MAX_FLOOD_FILL_PIXELS: usize = 512 * 512;

/// Flood fill from (x, y) with the given color.
/// Returns true if the fill completed, false if it was aborted due to size limit.
pub fn flood_fill(canvas: &mut Canvas, x: u32, y: u32, fill_color: Color, cmd: &mut Command) -> bool {
    let layer_idx = canvas.active_layer;
    let Some(layer) = canvas.active_layer_mut() else {
        return false;
    };
    if layer.locked || !layer.visible {
        return false;
    }
    let w = layer.buffer.width;
    let h = layer.buffer.height;
    let Some(&target_color) = layer.buffer.get_pixel(x, y) else {
        return false;
    };
    if target_color == fill_color {
        return true;
    }

    let mut queue = VecDeque::new();
    queue.push_back((x, y));
    let mut filled = 0usize;

    while let Some((px, py)) = queue.pop_front() {
        let Some(&current) = layer.buffer.get_pixel(px, py) else {
            continue;
        };
        if current != target_color {
            continue;
        }

        filled += 1;
        if filled > MAX_FLOOD_FILL_PIXELS {
            return false;
        }

        cmd.add_change(layer_idx, px, py, current, fill_color);
        layer.buffer.set_pixel(px, py, fill_color);

        if px > 0 {
            queue.push_back((px - 1, py));
        }
        if px + 1 < w {
            queue.push_back((px + 1, py));
        }
        if py > 0 {
            queue.push_back((px, py - 1));
        }
        if py + 1 < h {
            queue.push_back((px, py + 1));
        }
    }
    true
}

/// Draw a line using Bresenham's algorithm (shape tool, not pencil drag).
pub fn draw_line(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    pencil_line(canvas, x0, y0, x1, y1, color, cmd);
}

/// Compute the points of a line using Bresenham's algorithm (for preview).
pub fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;
    loop {
        points.push((cx, cy));
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
    points
}

/// Draw a rectangle outline.
pub fn draw_rect(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    // Top and bottom edges
    for x in min_x..=max_x {
        if x >= 0 && min_y >= 0 {
            pencil_pixel(canvas, x as u32, min_y as u32, color, cmd);
        }
        if x >= 0 && max_y >= 0 && max_y != min_y {
            pencil_pixel(canvas, x as u32, max_y as u32, color, cmd);
        }
    }
    // Left and right edges (excluding corners)
    for y in (min_y + 1)..max_y {
        if y >= 0 && min_x >= 0 {
            pencil_pixel(canvas, min_x as u32, y as u32, color, cmd);
        }
        if y >= 0 && max_x >= 0 && max_x != min_x {
            pencil_pixel(canvas, max_x as u32, y as u32, color, cmd);
        }
    }
}

/// Compute points of a rectangle outline (for preview).
pub fn rect_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    for x in min_x..=max_x {
        points.push((x, min_y));
        if max_y != min_y {
            points.push((x, max_y));
        }
    }
    for y in (min_y + 1)..max_y {
        points.push((min_x, y));
        if max_x != min_x {
            points.push((max_x, y));
        }
    }
    points
}

/// Draw an ellipse outline using the midpoint ellipse algorithm.
pub fn draw_ellipse(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    for (px, py) in ellipse_points(x0, y0, x1, y1) {
        if px >= 0 && py >= 0 {
            pencil_pixel(canvas, px as u32, py as u32, color, cmd);
        }
    }
}

/// Compute points of an ellipse outline (for preview).
pub fn ellipse_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    let cx2 = min_x + max_x; // 2 * center_x (keep integer math)
    let cy2 = min_y + max_y;
    let a = max_x - min_x; // diameter in x
    let b = max_y - min_y; // diameter in y

    if a == 0 && b == 0 {
        points.push((min_x, min_y));
        return points;
    }
    if a == 0 {
        for y in min_y..=max_y {
            points.push((min_x, y));
        }
        return points;
    }
    if b == 0 {
        for x in min_x..=max_x {
            points.push((x, min_y));
        }
        return points;
    }

    // Bresenham ellipse using integer arithmetic on the half-axes
    let a2 = (a * a) as i64;
    let b2 = (b * b) as i64;

    // We work in "doubled" coordinates to handle even/odd diameters
    let mut x = a as i64;
    let mut y = 0i64;

    let mut dx = b2 * (1 - 2 * a as i64);
    let mut dy = a2;
    let mut err = 0i64;

    // Plot 4 symmetric points from (cx2, cy2) in doubled coords
    let mut plot4 = |x: i64, y: i64| {
        let px1 = (cx2 as i64 + x) / 2;
        let py1 = (cy2 as i64 + y) / 2;
        let px2 = (cx2 as i64 - x) / 2;
        let py2 = (cy2 as i64 - y) / 2;
        // Use a set-like approach by just pushing - dedup later
        points.push((px1 as i32, py1 as i32));
        points.push((px2 as i32, py1 as i32));
        points.push((px1 as i32, py2 as i32));
        points.push((px2 as i32, py2 as i32));
    };

    // Region 1
    while b2 * x >= a2 * y {
        plot4(x, y);
        y += 1;
        err += dy;
        dy += 2 * a2;
        if 2 * err + dx > 0 {
            x -= 1;
            err += dx;
            dx += 2 * b2;
        }
    }
    // Region 2
    let mut x = 0i64;
    let mut y = b as i64;
    dx = b2;
    dy = a2 * (1 - 2 * b as i64);
    err = 0;
    while a2 * y >= b2 * x {
        plot4(x, y);
        x += 1;
        err += dx;
        dx += 2 * b2;
        if 2 * err + dy > 0 {
            y -= 1;
            err += dy;
            dy += 2 * a2;
        }
    }

    // Dedup
    points.sort();
    points.dedup();
    points
}

/// Draw a filled rectangle.
pub fn draw_filled_rect(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if x >= 0 && y >= 0 {
                pencil_pixel(canvas, x as u32, y as u32, color, cmd);
            }
        }
    }
}

/// Compute points of a filled rectangle (for preview).
pub fn filled_rect_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    let mut points = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            points.push((x, y));
        }
    }
    points
}

/// Draw a filled ellipse.
pub fn draw_filled_ellipse(
    canvas: &mut Canvas,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Color,
    cmd: &mut Command,
) {
    for (px, py) in filled_ellipse_points(x0, y0, x1, y1) {
        if px >= 0 && py >= 0 {
            pencil_pixel(canvas, px as u32, py as u32, color, cmd);
        }
    }
}

/// Compute points of a filled ellipse (for preview).
pub fn filled_ellipse_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    let cx = (min_x + max_x) as f64 / 2.0;
    let cy = (min_y + max_y) as f64 / 2.0;
    let rx = (max_x - min_x) as f64 / 2.0;
    let ry = (max_y - min_y) as f64 / 2.0;

    if rx == 0.0 && ry == 0.0 {
        return vec![(min_x, min_y)];
    }

    let mut points = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x as f64 - cx) / rx.max(0.5);
            let dy = (y as f64 - cy) / ry.max(0.5);
            if dx * dx + dy * dy <= 1.0001 {
                points.push((x, y));
            }
        }
    }
    points
}

/// Check if the active layer can be drawn on.
/// Returns `None` if drawable, or `Some(reason)` if not.
pub fn check_drawable(canvas: &Canvas) -> Option<&'static str> {
    match canvas.active_layer_ref() {
        None => Some("No active layer"),
        Some(layer) => {
            if layer.locked {
                Some("Layer is locked")
            } else if !layer.visible {
                Some("Layer is hidden")
            } else {
                None
            }
        }
    }
}

/// Eyedropper: pick color at (x, y) from the active layer.
pub fn eyedropper(canvas: &Canvas, x: u32, y: u32) -> Option<Color> {
    canvas
        .active_layer_ref()
        .and_then(|layer| layer.buffer.get_pixel(x, y).copied())
}

/// Apply an undo command to the canvas (revert changes).
pub fn apply_undo(canvas: &mut Canvas, cmd: &Command) {
    for change in cmd.changes.iter().rev() {
        if let Some(layer) = canvas.layers.get_mut(change.layer_index) {
            layer.buffer.set_pixel(change.x, change.y, change.old_color);
        }
    }
}

/// Apply a redo command to the canvas (re-apply changes).
pub fn apply_redo(canvas: &mut Canvas, cmd: &Command) {
    for change in &cmd.changes {
        if let Some(layer) = canvas.layers.get_mut(change.layer_index) {
            layer.buffer.set_pixel(change.x, change.y, change.new_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pencil_line() {
        let mut canvas = Canvas::new(8, 8);
        let fx = canvas.frame_x as i32;
        let fy = canvas.frame_y as i32;
        let mut cmd = Command::new("line");
        pencil_line(&mut canvas, fx, fy, fx + 3, fy, Color::WHITE, &mut cmd);
        assert_eq!(cmd.changes.len(), 4);
        for i in 0..4 {
            assert_eq!(
                canvas.layers[0].buffer.get_pixel(canvas.frame_x + i, canvas.frame_y),
                Some(&Color::WHITE)
            );
        }
    }

    #[test]
    fn test_flood_fill() {
        let mut canvas = Canvas::new(4, 4);
        let fx = canvas.frame_x;
        let fy = canvas.frame_y;
        let mut cmd = Command::new("fill");
        // Flood fill fills connected transparent area - entire buffer since all is transparent
        flood_fill(&mut canvas, fx, fy, Color::new(255, 0, 0, 255), &mut cmd);
        // All buffer pixels get filled (buffer is 12x12 = 144 for a 4x4 frame)
        assert!(cmd.changes.len() > 0);
    }

    #[test]
    fn test_eyedropper() {
        let mut canvas = Canvas::new(4, 4);
        let red = Color::new(255, 0, 0, 255);
        let bx = canvas.frame_x + 2;
        let by = canvas.frame_y + 2;
        canvas.layers[0].buffer.set_pixel(bx, by, red);
        assert_eq!(eyedropper(&canvas, bx, by), Some(red));
    }

    #[test]
    fn test_draw_line() {
        let mut canvas = Canvas::new(8, 8);
        let fx = canvas.frame_x as i32;
        let fy = canvas.frame_y as i32;
        let mut cmd = Command::new("line");
        draw_line(&mut canvas, fx, fy, fx + 4, fy, Color::WHITE, &mut cmd);
        assert_eq!(cmd.changes.len(), 5);
    }

    #[test]
    fn test_draw_rect() {
        let mut canvas = Canvas::new(8, 8);
        let fx = canvas.frame_x as i32;
        let fy = canvas.frame_y as i32;
        let mut cmd = Command::new("rect");
        draw_rect(&mut canvas, fx + 1, fy + 1, fx + 4, fy + 4, Color::WHITE, &mut cmd);
        // Perimeter of 4x4 rect: 4+4+2+2 = 12
        assert_eq!(cmd.changes.len(), 12);
    }

    #[test]
    fn test_draw_ellipse() {
        let mut canvas = Canvas::new(16, 16);
        let fx = canvas.frame_x as i32;
        let fy = canvas.frame_y as i32;
        let mut cmd = Command::new("ellipse");
        draw_ellipse(&mut canvas, fx + 2, fy + 2, fx + 10, fy + 8, Color::WHITE, &mut cmd);
        assert!(cmd.changes.len() > 0);
    }

    #[test]
    fn test_rect_points_preview() {
        let pts = rect_points(0, 0, 3, 3);
        assert_eq!(pts.len(), 12);
    }

    #[test]
    fn test_ellipse_points_degenerate() {
        let pts = ellipse_points(5, 5, 5, 5);
        assert_eq!(pts.len(), 1);
    }

    #[test]
    fn test_undo_redo_apply() {
        let mut canvas = Canvas::new(4, 4);
        let bx = canvas.frame_x + 1;
        let by = canvas.frame_y + 1;
        let mut cmd = Command::new("draw");
        pencil_pixel(&mut canvas, bx, by, Color::WHITE, &mut cmd);

        assert_eq!(canvas.layers[0].buffer.get_pixel(bx, by), Some(&Color::WHITE));
        apply_undo(&mut canvas, &cmd);
        assert_eq!(
            canvas.layers[0].buffer.get_pixel(bx, by),
            Some(&Color::TRANSPARENT)
        );
        apply_redo(&mut canvas, &cmd);
        assert_eq!(canvas.layers[0].buffer.get_pixel(bx, by), Some(&Color::WHITE));
    }
}
