//! Snapshot undo. Callers choose the state and group continuous edits.

pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<T> History<T> {
    pub fn record(&mut self, before: T) {
        self.redo.clear();
        self.undo.push(before);
    }

    pub fn undo(&mut self, current: T) -> Option<T> {
        self.undo.pop().inspect(|_| self.redo.push(current))
    }

    pub fn redo(&mut self, current: T) -> Option<T> {
        self.redo.pop().inspect(|_| self.undo.push(current))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_branch() {
        let mut history = History::default();
        assert!(!history.can_undo());
        history.record(1);
        assert_eq!(history.undo(2), Some(1));
        assert!(!history.can_undo());
        assert_eq!(history.redo(1), Some(2));
        assert!(!history.can_redo());
        assert_eq!(history.undo(2), Some(1));
        history.record(1);
        assert!(!history.can_redo());
        assert_eq!(history.undo(3), Some(1));
        assert_eq!(history.redo(1), Some(3));
    }
}
