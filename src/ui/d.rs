use crate::document::EditorWriter;
use crate::graph::{Id, Path, PlaceholderState};

pub enum TextStyle {
    Default,
    Keyword,
    TypeRef,
    Punctuation,
    Literal,
}

pub enum NodeDisplay {
    Named(String),
    Identicon(uuid::Uuid),
}

pub enum D {
    Block(Vec<D>),
    Line(Vec<D>),
    Indent(Box<D>),
    Spacing(f32),

    Text(String, TextStyle),
    LabelArrow,

    NodeHeader {
        path: Path,
        id: Id,
        display: NodeDisplay,
    },

    FieldLabel {
        entity_path: Path,
        label_id: Id,
    },

    CollapseToggle {
        path: Path,
        collapsed: bool,
    },

    StringEditor {
        path: Path,
        value: String,
    },

    NumberEditor {
        path: Path,
        value: f64,
        editing: Option<String>,
    },

    Placeholder {
        active: Option<ActivePlaceholder>,
    },

    List {
        opening: String,
        closing: String,
        separator: String,
        items: Vec<D>,
    },
}

pub struct ActivePlaceholder {
    pub state: PlaceholderState,
    pub on_commit: Box<dyn Fn(&mut EditorWriter, Id)>,
}
