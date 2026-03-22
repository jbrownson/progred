use crate::builtin_values::BuiltinValuesGid;
use crate::d::{DEvent, PlaceholderCommit};
use crate::document::Document;
use crate::generated::semantics::{ISA, list};
use crate::graph::{Gid, Id};
use crate::path::Path;
use crate::selection::{EdgeState, EditingState, GraphSelection, PlaceholderState, Selection};
use crate::spanningtree::SpanningTree;
use ordered_float::OrderedFloat;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Editor {
    pub doc: Document,
    pub semantics: Document,
    pub builtins: BuiltinValuesGid,
    pub tree: SpanningTree,
    pub selection: Option<Selection>,
    pub editing: Option<EditingState>,
    pub graph_selection: Option<GraphSelection>,
    pub file_path: Option<PathBuf>,
}

impl Editor {
    pub fn lib(&self) -> impl crate::graph::Gid + '_ {
        crate::graph::StackedGid::new(
            &self.doc.gid,
            crate::graph::StackedGid::new(&self.semantics.gid, &self.builtins),
        )
    }

    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            semantics: progred_macros::load_document!("../semantics.progred"),
            builtins: BuiltinValuesGid,
            tree: SpanningTree::empty(),
            selection: Some(Selection::edge(Path::root())),
            editing: None,
            graph_selection: None,
            file_path: None,
        }
    }

    pub fn render_d_tree(&self) -> crate::d::D {
        use crate::d::D;
        match &self.doc.root {
            Some(id) => crate::render::render(self, &Path::root(), id),
            None => D::Descend {
                    path: Path::root(),
                    selection: Selection::edge(Path::root()),
                    child: Box::new(D::Placeholder {
                        on_commit: Box::new(|w: &mut Editor, value| {
                            w.doc.set_edge(&Path::root(), value);
                        }),
                    }),
            }
        }
    }

    fn next_selection_from(&self, path: &Path) -> Option<Selection> {
        let d = self.render_d_tree();
        let nav = crate::navigate::collect_descends(&d);
        crate::navigate::first_placeholder_from(&nav, path)
            .or_else(|| crate::navigate::first_placeholder(&nav))
    }

    fn realize_placeholder(&mut self, commit: PlaceholderCommit) -> Id {
        match commit {
            PlaceholderCommit::Existing(id) => id,
            PlaceholderCommit::NewNode { isa } => {
                let uuid = uuid::Uuid::new_v4();
                self.doc.gid.set(uuid, crate::generated::semantics::ISA.into(), isa);
                Id::Uuid(uuid)
            }
        }
    }

    pub fn selected_node_id(&self) -> Option<Id> {
        if let Some(sel) = &self.selection {
            match sel {
                Selection::Edge(path, _) | Selection::ListElement { path, .. } => return self.doc.node(path),
                _ => {}
            }
        }
        match self.graph_selection.as_ref()? {
            GraphSelection::Edge { entity, label } => self.doc.gid.edges(entity).and_then(|e| e.get(label)).cloned(),
            GraphSelection::Node(id) => Some(id.clone()),
        }
    }

    pub fn handle_events(&mut self, events: Vec<DEvent<'_>>, mode: &InteractionMode) {
        for event in events {
            match event {
                DEvent::ClickedNode { id, selection } => match mode {
                    InteractionMode::Normal => {
                        self.selection = Some(selection);
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
                DEvent::ClickedPlaceholder(path) => {
                    self.editing = Some(EditingState { path: path.clone(), placeholder: PlaceholderState::default(), number_text: None });
                    self.selection = Some(Selection::edge(path));
                }
                DEvent::ClickedStringEditor(path) => {
                    self.editing = None;
                    self.selection = Some(Selection::edge(path));
                }
                DEvent::ClickedNumberEditor(path) => {
                    if let Some(Id::Number(n)) = self.doc.node(&path) {
                        self.editing = Some(EditingState { path: path.clone(), placeholder: PlaceholderState::default(), number_text: Some(n.to_string()) });
                        let mut es = EdgeState::default();
                        es.number_text = Some(n.to_string());
                        self.selection = Some(Selection::Edge(path, es));
                    }
                }
                DEvent::StringEditorStringChanged { path, text } => {
                    self.doc.set_edge(&path, Id::String(text));
                }
                DEvent::NumberEditorTextChanged { path, text } => {
                    if let Some(editing) = &mut self.editing {
                        editing.number_text = Some(text.clone());
                    }
                    if let Some(es) = self.selection.as_mut().and_then(|s| s.edge_state_mut()) {
                        es.number_text = Some(text.clone());
                    }
                    if let Ok(n) = text.parse::<f64>() {
                        self.doc.set_edge(&path, Id::Number(OrderedFloat(n)));
                    }
                }
                DEvent::PlaceholderCommitted { on_commit, value } => {
                    let focus_path = self.editing.as_ref().map(|e| &e.path)
                        .or_else(|| self.selection.as_ref().and_then(|s| s.path()))
                        .cloned();
                    let id = self.realize_placeholder(value);
                    on_commit(self, id);
                    self.editing = None;
                    self.selection = focus_path.and_then(|p| self.next_selection_from(&p));
                }
                DEvent::ListInsertCommitted { path, value } => {
                    let head_value = self.realize_placeholder(value);
                    if let Some(current_value) = self.doc.node(&path) {
                        let new_cons = Id::new_uuid();
                        self.doc.set_edge(&path, new_cons.clone());
                        self.doc.set_edge(&path.child(ISA.into()), list::Cons::<()>::TYPE_UUID.into());
                        self.doc.set_edge(&path.child(list::Cons::<()>::HEAD.into()), head_value);
                        self.doc.set_edge(&path.child(list::Cons::<()>::TAIL.into()), current_value);
                    }
                    self.editing = None;
                    self.selection = self.next_selection_from(&path);
                }
                DEvent::PlaceholderDismissed => {
                    self.editing = None;
                    self.selection = None;
                }
                DEvent::PlaceholderTextChanged(text) => {
                    if let Some(editing) = &mut self.editing {
                        editing.placeholder.text = text.clone();
                        editing.placeholder.selected_index = 0;
                    }
                    if let Some(es) = self.selection.as_mut().and_then(|s| s.edge_state_mut()) {
                        es.placeholder.text = text;
                        es.placeholder.selected_index = 0;
                    }
                }
                DEvent::PlaceholderSelectionMoved(index) => {
                    if let Some(editing) = &mut self.editing {
                        editing.placeholder.selected_index = index;
                    }
                    if let Some(es) = self.selection.as_mut().and_then(|s| s.edge_state_mut()) {
                        es.placeholder.selected_index = index;
                    }
                }
                DEvent::GraphNodeClicked(id) => {
                    self.graph_selection = Some(GraphSelection::Node(id));
                }
                DEvent::GraphEdgeClicked { entity, label } => {
                    self.graph_selection = Some(GraphSelection::Edge { entity, label });
                }
                DEvent::GraphBackgroundClicked => {
                    self.graph_selection = None;
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
        assert_eq!(name_of(&editor.semantics.gid, &Field::TYPE_UUID.into()), Some("field".to_string()));
    }

    #[test]
    fn lib_resolves_semantics() {
        let editor = Editor::new();
        assert_eq!(name_of(&editor.lib(), &Field::TYPE_UUID.into()), Some("field".to_string()));
    }

    #[test]
    fn lib_resolves_doc_node_with_semantics_type() {
        use crate::generated::semantics::{ISA, NAME};

        let mut editor = Editor::new();
        let uuid = uuid::Uuid::new_v4();
        let node = Id::Uuid(uuid);
        editor.doc.gid.set(uuid, ISA.into(), Field::TYPE_UUID.into());
        editor.doc.gid.set(uuid, NAME.into(), Id::String("age".into()));

        let lib = editor.lib();
        assert_eq!(lib.get(&node, &ISA.into()), Some(&Field::TYPE_UUID.into()));
        assert_eq!(display_label(&lib, &node), Some("field \"age\"".to_string()));
    }
}
