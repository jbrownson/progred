//! Snapshot undo over the persistent gid: cloning a `Document` is
//! O(1) structural sharing, so history is a dumb stack of
//! pre-mutation snapshots. Every mutation site records its own step
//! explicitly; text-run coalescing is not history's concern — the
//! run is the mounted editor's lifetime, and write-through reports
//! only the run's first write (see `raw::write_through`).

use crate::raw::{Document, Path};

/// A pre-mutation snapshot: the document, and the selection to
/// restore (an edge path; pendings restore as no selection — they
/// were disposable).
struct Entry {
    doc: Document,
    selection: Option<Path>,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    saved: usize,
}

impl History {
    /// Records a mutation the caller just made, `before` being the
    /// pre-mutation state.
    pub fn record(&mut self, before: Document, selection: Option<Path>) {
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
    pub fn mark_saved(&mut self) {
        self.saved = self.undo.len();
    }

    /// Modified since the save mark — a pure position comparison, so
    /// undoing back to the mark is clean again.
    pub fn dirty(&self) -> bool {
        self.undo.len() != self.saved
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
    use progred_graph::{Id, MutGid, new_node_id};

    fn doc(value: f64) -> Document {
        let mut gid = MutGid::new();
        let node = new_node_id();
        gid.set(node, Id::from("x"), Id::from(value));
        Document {
            root: Some(Id::from(node)),
            gid,
        }
    }

    fn x_of(doc: &Document) -> &Id {
        crate::raw::resolve(doc, &[Id::from("x")]).unwrap()
    }

    #[test]
    fn undo_and_redo_roundtrip_with_selection() {
        let mut history = History::default();
        let path = vec![Id::from("x")];
        history.record(doc(1.0), Some(path.clone()));

        let (back, selection) = history.undo(doc(2.0), None).unwrap();
        assert_eq!(x_of(&back), &Id::from(1.0));
        assert_eq!(selection, Some(path));

        let (forward, _) = history.redo(back, selection).unwrap();
        assert_eq!(x_of(&forward), &Id::from(2.0));
        assert!(history.redo(doc(9.9), None).is_none());
    }

    #[test]
    fn recording_clears_redo() {
        let mut history = History::default();
        let x = vec![Id::from("x")];
        history.record(doc(1.0), Some(x.clone()));
        let (back, _) = history.undo(doc(1.5), Some(x.clone())).unwrap();
        history.record(back, Some(x));
        assert!(history.redo(doc(0.0), None).is_none());
    }

    #[test]
    fn dirty_is_position_relative_to_the_save_mark() {
        let mut history = History::default();
        assert!(!history.dirty());
        let x = vec![Id::from("x")];
        history.record(doc(1.0), Some(x.clone()));
        assert!(history.dirty());

        history.mark_saved();
        assert!(!history.dirty());
        history.record(doc(1.2), Some(x.clone()));
        assert!(history.dirty());

        // Undoing back to the mark is clean; past it, dirty again.
        let (one_back, sel) = history.undo(doc(1.3), Some(x.clone())).unwrap();
        assert!(!history.dirty());
        let (_, _) = history.undo(one_back, sel).unwrap();
        assert!(history.dirty());
    }
}
