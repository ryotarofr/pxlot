use pixelforge_core::Canvas;
use serde::{Deserialize, Serialize};

/// A single animation frame holding a snapshot of all layers.
#[derive(Clone, Serialize, Deserialize)]
pub struct Frame {
    pub canvas: Canvas,
    pub delay_ms: u32,
}

impl Frame {
    pub fn new(canvas: Canvas, delay_ms: u32) -> Self {
        Self { canvas, delay_ms }
    }

    /// Flatten visible layers into RGBA bytes (frame region only, for export).
    pub fn flatten(&self) -> Vec<u8> {
        self.canvas.flatten_frame_visible()
    }

    /// Frame width (export dimensions).
    pub fn width(&self) -> u32 {
        self.canvas.frame_width()
    }

    /// Frame height (export dimensions).
    pub fn height(&self) -> u32 {
        self.canvas.frame_height()
    }
}

/// Timeline manages a sequence of animation frames.
#[derive(Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub frames: Vec<Frame>,
    pub current_frame: usize,
    pub fps: u32,
    pub playing: bool,
}

impl Timeline {
    pub fn new(initial_canvas: Canvas) -> Self {
        let frame = Frame::new(initial_canvas, 100);
        Self {
            frames: vec![frame],
            current_frame: 0,
            fps: 10,
            playing: false,
        }
    }

    /// Get the delay in ms for each frame based on FPS.
    pub fn frame_delay_ms(&self) -> u32 {
        if self.fps == 0 {
            return 1000;
        }
        1000 / self.fps
    }

    /// Get a reference to the current frame's canvas.
    pub fn current_canvas(&self) -> &Canvas {
        let idx = self.current_frame.min(self.frames.len().saturating_sub(1));
        &self.frames[idx].canvas
    }

    /// Get a mutable reference to the current frame's canvas.
    pub fn current_canvas_mut(&mut self) -> &mut Canvas {
        let idx = self.current_frame.min(self.frames.len().saturating_sub(1));
        &mut self.frames[idx].canvas
    }

    /// Add a new blank frame after the current frame.
    pub fn add_frame(&mut self, width: u32, height: u32) {
        let canvas = Canvas::new(width, height);
        let delay = self.frame_delay_ms();
        let frame = Frame::new(canvas, delay);
        let insert_idx = self.current_frame + 1;
        self.frames.insert(insert_idx, frame);
        self.current_frame = insert_idx;
    }

    /// Duplicate the current frame and insert after it.
    pub fn duplicate_frame(&mut self) {
        let cloned = self.frames[self.current_frame].clone();
        let insert_idx = self.current_frame + 1;
        self.frames.insert(insert_idx, cloned);
        self.current_frame = insert_idx;
    }

    /// Remove the current frame. Cannot remove the last frame.
    pub fn remove_frame(&mut self) -> bool {
        if self.frames.len() <= 1 {
            return false;
        }
        self.frames.remove(self.current_frame);
        if self.current_frame >= self.frames.len() {
            self.current_frame = self.frames.len() - 1;
        }
        true
    }

    /// Go to the next frame (wraps around).
    pub fn next_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
    }

    /// Go to the previous frame (wraps around).
    pub fn prev_frame(&mut self) {
        if self.current_frame == 0 {
            self.current_frame = self.frames.len() - 1;
        } else {
            self.current_frame -= 1;
        }
    }

    /// Select a specific frame by index.
    pub fn select_frame(&mut self, idx: usize) {
        if idx < self.frames.len() {
            self.current_frame = idx;
        }
    }

    /// Total frame count.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Toggle playback.
    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_timeline() {
        let canvas = Canvas::new(16, 16);
        let tl = Timeline::new(canvas);
        assert_eq!(tl.frame_count(), 1);
        assert_eq!(tl.current_frame, 0);
        assert_eq!(tl.fps, 10);
    }

    #[test]
    fn test_add_duplicate_remove() {
        let canvas = Canvas::new(8, 8);
        let mut tl = Timeline::new(canvas);

        tl.add_frame(8, 8);
        assert_eq!(tl.frame_count(), 2);
        assert_eq!(tl.current_frame, 1);

        tl.duplicate_frame();
        assert_eq!(tl.frame_count(), 3);
        assert_eq!(tl.current_frame, 2);

        assert!(tl.remove_frame());
        assert_eq!(tl.frame_count(), 2);

        // Remove until 1
        assert!(tl.remove_frame());
        assert_eq!(tl.frame_count(), 1);

        // Cannot remove last
        assert!(!tl.remove_frame());
    }

    #[test]
    fn test_navigation() {
        let canvas = Canvas::new(8, 8);
        let mut tl = Timeline::new(canvas);
        tl.add_frame(8, 8);
        tl.add_frame(8, 8);
        // Now at frame 2, total 3

        tl.select_frame(0);
        assert_eq!(tl.current_frame, 0);

        tl.next_frame();
        assert_eq!(tl.current_frame, 1);

        tl.next_frame();
        assert_eq!(tl.current_frame, 2);

        tl.next_frame(); // wraps
        assert_eq!(tl.current_frame, 0);

        tl.prev_frame(); // wraps back
        assert_eq!(tl.current_frame, 2);
    }

    #[test]
    fn test_frame_delay() {
        let canvas = Canvas::new(8, 8);
        let mut tl = Timeline::new(canvas);
        assert_eq!(tl.frame_delay_ms(), 100); // 1000/10

        tl.fps = 30;
        assert_eq!(tl.frame_delay_ms(), 33); // 1000/30
    }
}
