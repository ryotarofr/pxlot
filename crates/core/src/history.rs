use crate::Color;

/// A single pixel change for diff-based undo.
#[derive(Debug, Clone)]
pub struct PixelChange {
    pub layer_index: usize,
    pub x: u32,
    pub y: u32,
    pub old_color: Color,
    pub new_color: Color,
}

/// A command representing a group of pixel changes (one user action).
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub changes: Vec<PixelChange>,
}

impl Command {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            changes: Vec::new(),
        }
    }

    pub fn add_change(&mut self, layer_index: usize, x: u32, y: u32, old_color: Color, new_color: Color) {
        if old_color != new_color {
            self.changes.push(PixelChange {
                layer_index,
                x,
                y,
                old_color,
                new_color,
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Estimated memory usage of this command in bytes.
    pub fn byte_size(&self) -> usize {
        self.name.len() + self.changes.len() * std::mem::size_of::<PixelChange>()
    }
}

const MAX_MEMORY: usize = 128 * 1024 * 1024; // 128 MB
const MAX_COMMANDS: usize = 500;

/// Undo/Redo history manager.
pub struct History {
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    total_bytes: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Push a new command onto the undo stack.
    /// Clears the redo stack. Evicts oldest commands if limits are exceeded.
    pub fn push(&mut self, cmd: Command) {
        if cmd.is_empty() {
            return;
        }
        // Clear redo stack
        for c in self.redo_stack.drain(..) {
            self.total_bytes = self.total_bytes.saturating_sub(c.byte_size());
        }

        self.total_bytes += cmd.byte_size();
        self.undo_stack.push(cmd);

        // Evict oldest if over limits
        while (self.undo_stack.len() > MAX_COMMANDS || self.total_bytes > MAX_MEMORY)
            && self.undo_stack.len() > 1
        {
            if let Some(evicted) = self.undo_stack.first() {
                self.total_bytes = self.total_bytes.saturating_sub(evicted.byte_size());
            }
            self.undo_stack.remove(0);
        }
    }

    /// Pop the most recent command for undo. Returns changes to revert.
    pub fn undo(&mut self) -> Option<&Command> {
        let cmd = self.undo_stack.pop()?;
        // Bytes stay the same (moved from undo to redo stack)
        self.redo_stack.push(cmd);
        self.redo_stack.last()
    }

    /// Pop the most recent redo command. Returns changes to re-apply.
    pub fn redo(&mut self) -> Option<&Command> {
        let cmd = self.redo_stack.pop()?;
        // Bytes stay the same (moved from redo to undo stack)
        self.undo_stack.push(cmd);
        self.undo_stack.last()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn memory_usage(&self) -> usize {
        self.total_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_redo() {
        let mut history = History::new();
        let mut cmd = Command::new("draw");
        cmd.add_change(0, 1, 1, Color::TRANSPARENT, Color::BLACK);
        history.push(cmd);

        assert!(history.can_undo());
        assert!(!history.can_redo());

        let undone = history.undo().unwrap();
        assert_eq!(undone.changes.len(), 1);
        assert!(history.can_redo());
        assert!(!history.can_undo());

        let redone = history.redo().unwrap();
        assert_eq!(redone.changes.len(), 1);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_push_clears_redo() {
        let mut history = History::new();
        let mut cmd1 = Command::new("draw1");
        cmd1.add_change(0, 0, 0, Color::TRANSPARENT, Color::BLACK);
        history.push(cmd1);

        history.undo();
        assert!(history.can_redo());

        let mut cmd2 = Command::new("draw2");
        cmd2.add_change(0, 1, 1, Color::TRANSPARENT, Color::WHITE);
        history.push(cmd2);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_empty_command_not_pushed() {
        let mut history = History::new();
        let cmd = Command::new("empty");
        history.push(cmd);
        assert!(!history.can_undo());
    }
}
