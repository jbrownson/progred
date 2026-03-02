use crate::d::DEvent;
use crate::document::Document;
use crate::graph::{EdgeState, Gid, Id, Path, RootSlot, Selection, SpanningTree};
use ordered_float::OrderedFloat;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Editor {
    pub doc: Document,
    pub semantics: Document,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub file_path: Option<PathBuf>,
}

impl Editor {
    pub fn lib(&self) -> crate::graph::StackedGid<'_, crate::graph::MutGid, crate::graph::MutGid> {
        crate::graph::StackedGid::new(&self.doc.gid, &self.semantics.gid)
    }

    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            semantics: progred_macros::load_document!("../semantics.progred"),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
        }
    }

    pub fn selected_node_id(&self) -> Option<Id> {
        match self.selection.as_ref()? {
            Selection::Edge(path, _) => self.doc.node(path),
            Selection::GraphEdge { entity, label } => self.doc.gid.edges(entity).and_then(|e| e.get(label)).cloned(),
            Selection::GraphNode(id) => Some(id.clone()),
            Selection::InsertRoot(..) => None,
        }
    }

    pub fn handle_events(&mut self, events: Vec<DEvent<'_>>, mode: &InteractionMode) {
        for event in events {
            match event {
                DEvent::ClickedNode { path, id } => match mode {
                    InteractionMode::Normal => {
                        self.selection = Some(Selection::edge(path));
                    }
                    InteractionMode::Assign(target) => {
                        self.doc.set_edge(target, id);
                        self.selection = None;
                    }
                    InteractionMode::SelectUnder(source) => {
                        self.selection = Some(Selection::edge(source.child(id)));
                    }
                },
                DEvent::ClickedFieldLabel { entity_path: _, label_id } => match mode {
                    InteractionMode::Normal => {}
                    InteractionMode::Assign(target) => {
                        self.doc.set_edge(target, label_id);
                        self.selection = None;
                    }
                    InteractionMode::SelectUnder(source) => {
                        self.selection = Some(Selection::edge(source.child(label_id)));
                    }
                },
                DEvent::ClickedCollapseToggle(path) => {
                    let collapsed = self.tree.is_collapsed(&path).unwrap_or(false);
                    self.tree.set_collapsed(&path, !collapsed);
                }
                DEvent::ClickedBackground => {
                    self.selection = None;
                }
                DEvent::ClickedRootInsertionPoint(index) => {
                    self.selection = Some(Selection::insert_root(index));
                }
                DEvent::ClickedStringEditor(path) => {
                    self.selection = Some(Selection::edge(path));
                }
                DEvent::ClickedNumberEditor(path) => {
                    if let Some(Id::Number(n)) = self.doc.node(&path) {
                        let mut es = EdgeState::default();
                        es.number_text = Some(n.to_string());
                        self.selection = Some(Selection::Edge(path, es));
                    }
                }
                DEvent::StringEditorStringChanged { path, text } => {
                    self.doc.set_edge(&path, Id::String(text));
                }
                DEvent::NumberEditorTextChanged { path, text } => {
                    if let Some(Selection::Edge(_, ref mut es)) = self.selection {
                        es.number_text = Some(text.clone());
                    }
                    if let Ok(n) = text.parse::<f64>() {
                        self.doc.set_edge(&path, Id::Number(OrderedFloat(n)));
                    }
                }
                DEvent::PlaceholderCommitted { on_commit, value } => {
                    on_commit(self, value);
                    self.selection = None;
                }
                DEvent::PlaceholderDismissed => {
                    self.selection = None;
                }
                DEvent::PlaceholderTextChanged(text) => {
                    if let Some(Selection::Edge(_, ref mut es)) = self.selection {
                        es.placeholder.text = text;
                        es.placeholder.selected_index = 0;
                    }
                }
                DEvent::PlaceholderSelectionMoved(index) => {
                    if let Some(Selection::Edge(_, ref mut es)) = self.selection {
                        es.placeholder.selected_index = index;
                    }
                }
                DEvent::RootPlaceholderCommitted { index, value } => {
                    self.doc.roots.insert(index, RootSlot::new(value));
                    self.selection = None;
                }
                DEvent::RootPlaceholderDismissed => {
                    self.selection = None;
                }
                DEvent::RootPlaceholderTextChanged(text) => {
                    if let Some(Selection::InsertRoot(_, ref mut ps)) = self.selection {
                        ps.text = text;
                        ps.selected_index = 0;
                    }
                }
                DEvent::RootPlaceholderSelectionMoved(index) => {
                    if let Some(Selection::InsertRoot(_, ref mut ps)) = self.selection {
                        ps.selected_index = index;
                    }
                }
                DEvent::GraphNodeClicked(id) => {
                    self.selection = Some(Selection::GraphNode(id));
                }
                DEvent::GraphEdgeClicked { entity, label } => {
                    self.selection = Some(Selection::GraphEdge { entity, label });
                }
                DEvent::GraphBackgroundClicked => {
                    self.selection = None;
                }
            }
        }
    }
}

pub enum InteractionMode {
    Normal,
    SelectUnder(Path),
    Assign(Path),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{display_label, name_of};
    use crate::generated::semantics::Field;
    use crate::graph::Gid;

    #[test]
    fn semantics_contains_field_type() {
        let editor = Editor::new();
        assert_eq!(name_of(&editor.semantics.gid, &Field::TYPE_ID), Some("field".to_string()));
    }

    #[test]
    fn lib_resolves_semantics() {
        let editor = Editor::new();
        assert_eq!(name_of(&editor.lib(), &Field::TYPE_ID), Some("field".to_string()));
    }

    #[test]
    fn lib_resolves_doc_node_with_semantics_type() {
        use crate::generated::semantics::{ISA, NAME};

        let mut editor = Editor::new();
        let uuid = uuid::Uuid::new_v4();
        let node = Id::Uuid(uuid);
        editor.doc.gid.set(uuid, ISA.clone(), Field::TYPE_ID.clone());
        editor.doc.gid.set(uuid, NAME.clone(), Id::String("age".into()));

        let lib = editor.lib();
        assert_eq!(lib.get(&node, &ISA), Some(&Field::TYPE_ID));
        assert_eq!(display_label(&lib, &node), Some("field \"age\"".to_string()));
    }
}
