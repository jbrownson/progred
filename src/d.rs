use crate::document::EditorWriter;
use crate::graph::{Id, Path, PlaceholderState};

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

    Descend { path: Path, id: Id, child: Box<D> },

    NodeHeader { child: Box<D> },
    FieldLabel { label_id: Id },
    // TODO: consider whether collapse belongs in D or should be a UI-layer concern
    CollapseToggle { collapsed: bool },
    StringEditor { value: String },
    NumberEditor { value: f64, editing: Option<String> },

    Placeholder {
        active: Option<ActivePlaceholder>,
    },

    List {
        opening: String,
        closing: String,
        separator: String,
        items: Vec<D>,
        vertical: bool,
    },
}

pub struct ActivePlaceholder {
    pub state: PlaceholderState,
    pub on_commit: Box<dyn Fn(&mut EditorWriter, Id)>,
}
