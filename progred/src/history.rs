//! Snapshot undo over the persistent gid: cloning a `Document` is
//! O(1) structural sharing, so history is a dumb stack of
//! pre-mutation snapshots. Every mutation site records its own step
//! explicitly; text-run coalescing is not history's concern — the
//! run is the mounted editor's lifetime, and write-through reports
//! only the run's first write (see `selection::write_through`).

use gid::{Document, Path};

/// A pre-mutation snapshot: the document, and the selection to
/// restore (an edge path; pendings restore as no selection — they
/// were disposable).
struct Entry {
    doc: Document,
    selection: Option<Path>,
}

pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    saved: Option<usize>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            saved: Some(0),
        }
    }
}

impl History {
    /// Records a mutation the caller just made, `before` being the
    /// pre-mutation state.
    pub fn record(&mut self, before: Document, selection: Option<Path>) {
        self.saved = self.saved.filter(|saved| *saved <= self.undo.len());
        self.redo.clear();
        self.undo.push(Entry {
            doc: before,
            selection,
        });
    }

    /// Steps back, exchanging the current state into the redo stack.
    pub fn undo(
        &mut self,
        current: Document,
        selection: Option<Path>,
    ) -> Option<(Document, Option<Path>)> {
        let entry = self.undo.pop()?;
        self.redo.push(Entry {
            doc: current,
            selection,
        });
        Some((entry.doc, entry.selection))
    }

    /// Steps forward again; a new recording clears this path.
    pub fn redo(
        &mut self,
        current: Document,
        selection: Option<Path>,
    ) -> Option<(Document, Option<Path>)> {
        let entry = self.redo.pop()?;
        self.undo.push(Entry {
            doc: current,
            selection,
        });
        Some((entry.doc, entry.selection))
    }

    /// Marks the current position as saved. The caller breaks any
    /// open edit run at the selection, keeping runs off the mark.
    #[cfg(any(test, target_os = "macos", target_os = "linux"))]
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.undo.len());
    }

    /// Modified since the save mark. A discarded mark stays dirty
    /// until the next save.
    pub fn dirty(&self) -> bool {
        Some(self.undo.len()) != self.saved
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, Step, Value, new_cell_id};

    fn x() -> Step {
        Step::Key(crate::test_values::label("x"))
    }

    fn doc(value: &str) -> Document {
        let mut cells = Cells::new();
        let cell = new_cell_id();
        cells.set_value(
            cell,
            Value::record([(
                crate::test_values::label("x"),
                crate::test_values::text(value),
            )]),
        );
        Document {
            root: Some(Value::from(cell)),
            cells,
        }
    }

    fn x_of(doc: &Document) -> Value {
        let libraries = progred_libraries::Libraries::default();
        crate::sources::Sources {
            doc,
            libraries: &libraries,
        }
        .resolve_path(&[Step::Follow(gid::Resolution::Document), x()])
        .unwrap()
        .clone()
    }

    #[test]
    fn undo_and_redo_roundtrip_with_selection() {
        let mut history = History::default();
        let path = vec![Step::Follow(gid::Resolution::Document), x()];
        history.record(doc("1"), Some(path.clone()));

        let (back, selection) = history.undo(doc("2"), None).unwrap();
        assert_eq!(x_of(&back), crate::test_values::text("1"));
        assert_eq!(selection, Some(path));

        let (forward, _) = history.redo(back, selection).unwrap();
        assert_eq!(x_of(&forward), crate::test_values::text("2"));
        assert!(history.redo(doc("9"), None).is_none());
    }

    #[test]
    fn recording_clears_redo() {
        let mut history = History::default();
        let x = vec![Step::Follow(gid::Resolution::Document), x()];
        history.record(doc("1"), Some(x.clone()));
        let (back, _) = history.undo(doc("1.5"), Some(x.clone())).unwrap();
        history.record(back, Some(x));
        assert!(history.redo(doc("0"), None).is_none());
    }

    #[test]
    fn dirty_is_position_relative_to_the_save_mark() {
        let mut history = History::default();
        assert!(!history.dirty());
        let x = vec![Step::Follow(gid::Resolution::Document), x()];
        history.record(doc("1"), Some(x.clone()));
        assert!(history.dirty());

        history.mark_saved();
        assert!(!history.dirty());
        history.record(doc("1.2"), Some(x.clone()));
        assert!(history.dirty());

        // Undoing back to the mark is clean; past it, dirty again.
        let (one_back, sel) = history.undo(doc("1.3"), Some(x.clone())).unwrap();
        assert!(!history.dirty());
        let (_, _) = history.undo(one_back, sel).unwrap();
        assert!(history.dirty());
    }

    #[test]
    fn branching_before_the_save_mark_stays_dirty_until_saved_again() {
        let mut history = History::default();
        history.record(doc("original"), None);
        history.mark_saved();

        let (original, selection) = history.undo(doc("saved"), None).unwrap();
        history.record(original, selection);
        assert!(history.dirty());
        assert!(!history.can_redo());

        let (original, selection) = history.undo(doc("different"), None).unwrap();
        assert!(history.dirty());
        let (different, _) = history.redo(original, selection).unwrap();
        assert_eq!(x_of(&different), crate::test_values::text("different"));
        assert!(history.dirty());

        history.mark_saved();
        assert!(!history.dirty());
    }

    #[test]
    fn branching_at_the_save_mark_preserves_it() {
        let mut history = History::default();
        history.record(doc("original"), None);
        history.mark_saved();
        history.record(doc("saved"), None);

        let (saved, selection) = history.undo(doc("discarded"), None).unwrap();
        history.record(saved, selection);
        assert!(history.dirty());

        let (saved, selection) = history.undo(doc("different"), None).unwrap();
        assert_eq!(x_of(&saved), crate::test_values::text("saved"));
        assert!(!history.dirty());
        history.redo(saved, selection).unwrap();
        assert!(history.dirty());
    }

    #[test]
    fn branching_after_the_save_mark_preserves_it() {
        let mut history = History::default();
        history.record(doc("original"), None);
        history.mark_saved();
        history.record(doc("saved"), None);
        history.record(doc("later"), None);

        let (later, selection) = history.undo(doc("discarded"), None).unwrap();
        history.record(later, selection);
        assert!(history.dirty());

        let (later, selection) = history.undo(doc("different"), None).unwrap();
        assert!(history.dirty());
        let (saved, _) = history.undo(later, selection).unwrap();
        assert_eq!(x_of(&saved), crate::test_values::text("saved"));
        assert!(!history.dirty());
    }
}
