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
mod tests;
