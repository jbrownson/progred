use crate::editor::Editor;
use crate::graph::Id;
use crate::path::Path;
use crate::selection::Selection;

pub enum PlaceholderCommit {
    Existing(Id),
    NewNode { isa: Id },
}

pub enum TextStyle {
    Keyword,
    TypeRef,
    Punctuation,
    Literal,
}

pub enum D {
    Block(Vec<D>),
    Line(Vec<D>),
    Indent(Box<D>),

    Text(String, TextStyle),
    Identicon(uuid::Uuid),

    Descend { path: Path, selection: Selection, child: Box<D> },

    NodeHeader { child: Box<D> },
    FieldLabel { label_id: Id },
    CollapseToggle { collapsed: bool },
    StringEditor { value: String },
    NumberEditor { value: f64, number_text: Option<String> },

    Placeholder {
        on_commit: Box<dyn Fn(&mut Editor, Id)>,
    },

    VerticalList {
        elements: Vec<D>,
    },
    HorizontalList {
        opening: String,
        closing: String,
        separator: String,
        elements: Vec<D>,
    },
}

pub enum DEvent<'a> {
    ClickedNode { id: Id, selection: Selection },
    ClickedFieldLabel { entity_path: Path, label_id: Id },
    ClickedCollapseToggle(Path),
    ClickedBackground,
    ClickedRootInsertionPoint(usize),

    ClickedStringEditor(Path),
    ClickedNumberEditor(Path),

    StringEditorStringChanged { path: Path, text: String },
    NumberEditorTextChanged { path: Path, text: String },

    PlaceholderCommitted { on_commit: &'a dyn Fn(&mut Editor, Id), value: PlaceholderCommit },
    PlaceholderDismissed,
    PlaceholderTextChanged(String),
    PlaceholderSelectionMoved(usize),

    RootPlaceholderCommitted { index: usize, value: PlaceholderCommit },
    RootPlaceholderDismissed,
    RootPlaceholderTextChanged(String),
    RootPlaceholderSelectionMoved(usize),

    ClickedListSlot(Path),
    ListSlotCommitted { path: Path, value: PlaceholderCommit },
    ListSlotDismissed,
    ListSlotTextChanged(String),
    ListSlotSelectionMoved(usize),

    GraphNodeClicked(Id),
    GraphEdgeClicked { entity: Id, label: Id },
    GraphBackgroundClicked,
}
