use crate::editor::EditorWriter;
use crate::graph::{Id, Path};

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

    Descend { path: Path, child: Box<D> },

    NodeHeader { child: Box<D> },
    FieldLabel { label_id: Id },
    // TODO: consider whether collapse belongs in D or should be a UI-layer concern
    CollapseToggle { collapsed: bool },
    StringEditor { value: String },
    NumberEditor { value: f64, editing: Option<String> },

    Placeholder {
        on_commit: Box<dyn Fn(&mut EditorWriter, Id)>,
    },

    List {
        opening: String,
        closing: String,
        separator: String,
        items: Vec<D>,
        vertical: bool,
    },
}

