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
