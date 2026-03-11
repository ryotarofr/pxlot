/// Tracks which rectangular area of a canvas has been modified,
/// enabling partial re-rendering for large canvases (512px+).
#[derive(Debug, Clone)]
pub struct DirtyRegion {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    dirty: bool,
}

impl DirtyRegion {
    /// Create a new clean (non-dirty) region.
    pub fn new() -> Self {
        Self {
            min_x: u32::MAX,
            min_y: u32::MAX,
            max_x: 0,
            max_y: 0,
            dirty: false,
        }
    }

    /// Mark a single pixel as dirty.
    pub fn mark_dirty(&mut self, x: u32, y: u32) {
        if self.dirty {
            self.min_x = self.min_x.min(x);
            self.min_y = self.min_y.min(y);
            self.max_x = self.max_x.max(x);
            self.max_y = self.max_y.max(y);
        } else {
            self.min_x = x;
            self.min_y = y;
            self.max_x = x;
            self.max_y = y;
            self.dirty = true;
        }
    }

    /// Mark a rectangular region as dirty (inclusive coordinates).
    pub fn mark_rect(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) {
        let rect_min_x = x1.min(x2);
        let rect_min_y = y1.min(y2);
        let rect_max_x = x1.max(x2);
        let rect_max_y = y1.max(y2);

        if self.dirty {
            self.min_x = self.min_x.min(rect_min_x);
            self.min_y = self.min_y.min(rect_min_y);
            self.max_x = self.max_x.max(rect_max_x);
            self.max_y = self.max_y.max(rect_max_y);
        } else {
            self.min_x = rect_min_x;
            self.min_y = rect_min_y;
            self.max_x = rect_max_x;
            self.max_y = rect_max_y;
            self.dirty = true;
        }
    }

    /// Clear the dirty region, marking everything as clean.
    pub fn clear(&mut self) {
        self.min_x = u32::MAX;
        self.min_y = u32::MAX;
        self.max_x = 0;
        self.max_y = 0;
        self.dirty = false;
    }

    /// Returns `true` if any region has been marked dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the bounding rectangle of all dirty pixels as
    /// `(min_x, min_y, max_x, max_y)` with inclusive coordinates,
    /// or `None` if nothing is dirty.
    pub fn bounds(&self) -> Option<(u32, u32, u32, u32)> {
        if self.dirty {
            Some((self.min_x, self.min_y, self.max_x, self.max_y))
        } else {
            None
        }
    }
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_region_is_clean() {
        let r = DirtyRegion::new();
        assert!(!r.is_dirty());
        assert_eq!(r.bounds(), None);
    }

    #[test]
    fn mark_single_pixel() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(10, 20);
        assert!(r.is_dirty());
        assert_eq!(r.bounds(), Some((10, 20, 10, 20)));
    }

    #[test]
    fn mark_multiple_pixels_expands_bounds() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(5, 10);
        r.mark_dirty(20, 3);
        r.mark_dirty(8, 30);
        assert_eq!(r.bounds(), Some((5, 3, 20, 30)));
    }

    #[test]
    fn mark_rect_basic() {
        let mut r = DirtyRegion::new();
        r.mark_rect(10, 20, 30, 40);
        assert!(r.is_dirty());
        assert_eq!(r.bounds(), Some((10, 20, 30, 40)));
    }

    #[test]
    fn mark_rect_swapped_coordinates() {
        let mut r = DirtyRegion::new();
        r.mark_rect(30, 40, 10, 20);
        assert_eq!(r.bounds(), Some((10, 20, 30, 40)));
    }

    #[test]
    fn mark_rect_merges_with_existing() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(5, 5);
        r.mark_rect(10, 10, 20, 20);
        assert_eq!(r.bounds(), Some((5, 5, 20, 20)));
    }

    #[test]
    fn clear_resets_state() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(10, 20);
        r.mark_rect(0, 0, 100, 100);
        assert!(r.is_dirty());

        r.clear();
        assert!(!r.is_dirty());
        assert_eq!(r.bounds(), None);
    }

    #[test]
    fn dirty_after_clear_then_mark() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(10, 20);
        r.clear();
        r.mark_dirty(50, 60);
        assert!(r.is_dirty());
        assert_eq!(r.bounds(), Some((50, 60, 50, 60)));
    }

    #[test]
    fn default_is_clean() {
        let r = DirtyRegion::default();
        assert!(!r.is_dirty());
        assert_eq!(r.bounds(), None);
    }

    #[test]
    fn mark_rect_single_pixel() {
        let mut r = DirtyRegion::new();
        r.mark_rect(7, 7, 7, 7);
        assert_eq!(r.bounds(), Some((7, 7, 7, 7)));
    }

    #[test]
    fn mark_dirty_at_zero() {
        let mut r = DirtyRegion::new();
        r.mark_dirty(0, 0);
        assert!(r.is_dirty());
        assert_eq!(r.bounds(), Some((0, 0, 0, 0)));
    }

    #[test]
    fn multiple_rects_merge() {
        let mut r = DirtyRegion::new();
        r.mark_rect(10, 10, 20, 20);
        r.mark_rect(50, 50, 60, 60);
        assert_eq!(r.bounds(), Some((10, 10, 60, 60)));
    }
}
