use pxlot_animation::Timeline;
use pxlot_core::history::History;
use pxlot_core::{Canvas, Color};
use pxlot_tools::ToolKind;

/// Application-wide editor state.
pub struct EditorState {
    pub canvas: Canvas,
    pub history: History,
    pub timeline: Timeline,
    pub current_tool: ToolKind,
    pub current_color: Color,
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub show_grid: bool,
    pub is_panning: bool,
    pub is_drawing: bool,
    pub last_draw_x: i32,
    pub last_draw_y: i32,
    pub shape_start_x: i32,
    pub shape_start_y: i32,
    pub color_history: Vec<Color>,
    pub playback_interval: Option<i32>,
    /// Last mouse screen position during panning.
    pub pan_last_mouse_x: f64,
    pub pan_last_mouse_y: f64,
    /// Selection rectangle (x, y, w, h) in pixel coords. None = no selection.
    pub selection: Option<(i32, i32, i32, i32)>,
    /// Secondary (background) color for X-key swap.
    pub secondary_color: Color,
    /// Mirror/symmetry drawing mode.
    pub mirror_x: bool,
    /// Custom grid cell size (in pixels).
    pub grid_size: u32,
    /// Clipboard for copy/paste (pixel data + dimensions).
    pub clipboard: Option<ClipboardData>,
    /// Transient status message shown briefly to the user (warnings, errors).
    pub status_message: Option<String>,
    /// Onion skin: show previous/next frames as translucent overlay.
    pub onion_skin: bool,
    /// Number of onion skin frames to show before/after current.
    pub onion_skin_frames: u32,
    /// Flag to center the view on the frame on the next render.
    pub needs_center: bool,
    /// Current hover pixel position in buffer coordinates (-1 = outside).
    pub hover_x: i32,
    pub hover_y: i32,
    /// Whether the space key is held (for space+drag panning).
    pub space_held: bool,
}

/// Clipboard data for copy/paste operations.
#[derive(Clone)]
pub struct ClipboardData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Color>,
}

impl EditorState {
    pub fn new(width: u32, height: u32) -> Self {
        let canvas = Canvas::new(width, height);
        let timeline = Timeline::new(canvas.clone());
        Self {
            canvas,
            history: History::new(),
            timeline,
            current_tool: ToolKind::Pencil,
            current_color: Color::WHITE,
            zoom: 16.0,
            pan_x: 0.0,
            pan_y: 0.0,
            show_grid: true,
            is_panning: false,
            is_drawing: false,
            last_draw_x: -1,
            last_draw_y: -1,
            shape_start_x: -1,
            shape_start_y: -1,
            color_history: Vec::new(),
            playback_interval: None,
            pan_last_mouse_x: 0.0,
            pan_last_mouse_y: 0.0,
            selection: None,
            secondary_color: Color::BLACK,
            mirror_x: false,
            grid_size: 1,
            clipboard: None,
            status_message: None,
            onion_skin: false,
            onion_skin_frames: 1,
            needs_center: true,
            hover_x: -1,
            hover_y: -1,
            space_held: false,
        }
    }

    /// Center the view on the frame area given the viewport dimensions.
    pub fn center_on_frame(&mut self, viewport_w: f64, viewport_h: f64) {
        let frame_cx = (self.canvas.frame_x as f64 + self.canvas.frame_width() as f64 / 2.0) * self.zoom;
        let frame_cy = (self.canvas.frame_y as f64 + self.canvas.frame_height() as f64 / 2.0) * self.zoom;
        self.pan_x = viewport_w / 2.0 - frame_cx;
        self.pan_y = viewport_h / 2.0 - frame_cy;
    }

    /// Save the current canvas state back to the timeline's current frame.
    pub fn save_frame(&mut self) {
        self.timeline.frames[self.timeline.current_frame].canvas = self.canvas.clone();
    }

    /// Switch to a different frame, saving the current one first.
    pub fn switch_frame(&mut self, idx: usize) {
        self.save_frame();
        self.timeline.select_frame(idx);
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
    }

    /// Add a new frame and switch to it.
    pub fn add_frame(&mut self) {
        let w = self.canvas.frame_width();
        let h = self.canvas.frame_height();
        self.save_frame();
        self.timeline.add_frame(w, h);
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
    }

    /// Duplicate the current frame and switch to the copy.
    pub fn duplicate_frame(&mut self) {
        self.save_frame();
        self.timeline.duplicate_frame();
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
    }

    /// Remove the current frame.
    pub fn remove_frame(&mut self) -> bool {
        if !self.timeline.remove_frame() {
            return false;
        }
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
        true
    }

    /// Go to the previous frame (wrapping), saving current first.
    pub fn prev_frame(&mut self) {
        self.save_frame();
        self.timeline.prev_frame();
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
    }

    /// Go to the next frame (wrapping), saving current first.
    pub fn next_frame(&mut self) {
        self.save_frame();
        self.timeline.next_frame();
        self.canvas = self.timeline.current_canvas().clone();
        self.history = History::new();
    }

    /// Record a color in the history (max 32).
    pub fn record_color(&mut self, color: Color) {
        if color == Color::TRANSPARENT {
            return;
        }
        // Remove if already present, then push to front
        self.color_history.retain(|c| *c != color);
        self.color_history.insert(0, color);
        if self.color_history.len() > 32 {
            self.color_history.truncate(32);
        }
    }
}
