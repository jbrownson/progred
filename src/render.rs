use crate::document::Editor;
use crate::generated::semantics::{Apply, Field, ARGS, CONS_TYPE, HEAD, ISA, NAME, TAIL, TYPE_};
use crate::graph::{EdgeState, Gid, Id, Path, Selection};
use crate::list_iter::ListIter;

use crate::d::{ActivePlaceholder, D, TextStyle};

pub fn render(editor: &Editor, path: &Path, id: &Id) -> D {
    render_id(editor, path, id, im::HashSet::new())
}

fn render_id(editor: &Editor, path: &Path, id: &Id, ancestors: im::HashSet<Id>) -> D {
    let child = render_id_inner(editor, path, id, ancestors);
    D::Descend { path: path.clone(), id: id.clone(), child: Box::new(child) }
}

fn render_id_inner(editor: &Editor, path: &Path, id: &Id, ancestors: im::HashSet<Id>) -> D {
    match id {
        Id::Uuid(_) if editor.is_list(id) => render_list(editor, path, id, ancestors),
        Id::Uuid(uuid) => {
            let ctx = RenderCtx { editor, path, id, ancestors: &ancestors };
            try_domain_render(&ctx)
                .unwrap_or_else(|| render_uuid(editor, path, uuid, ancestors))
        }
        Id::String(s) => D::StringEditor {
            value: s.clone(),
        },
        Id::Number(n) => D::NumberEditor {
            value: n.0,
            editing: editing_state(editor, path),
        },
    }
}

struct RenderCtx<'a> {
    editor: &'a Editor,
    path: &'a Path,
    id: &'a Id,
    ancestors: &'a im::HashSet<Id>,
}

impl<'a> RenderCtx<'a> {
    fn descend(&self, label: &Id) -> D {
        let child_path = self.path.child(label.clone());

        match self.editor.doc.gid.get(self.id, label) {
            Some(child_id) => {
                render_id(self.editor, &child_path, child_id, self.ancestors.clone())
            }
            None => {
                let commit_path = child_path.clone();
                D::Placeholder {
                    active: placeholder_active(self.editor, &child_path, move |w, value| {
                        w.set_edge(&commit_path, value);
                    }),
                }
            }
        }
    }
}

fn render_uuid(
    editor: &Editor,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: im::HashSet<Id>,
) -> D {
    let id = Id::Uuid(*uuid);
    let edges = editor.doc.gid.edges(&id);
    let display_label = editor.display_label(&id);
    let new_edge_label = editor.selection.as_ref()
        .and_then(|s| s.edge_path())
        .and_then(|sel| sel.pop())
        .filter(|(parent, _)| parent == path)
        .map(|(_, label)| label)
        .filter(|label| !edges.is_some_and(|e| e.contains_key(label)));
    let all_edges: Vec<(Id, Id)> = edges.into_iter()
        .flat_map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())))
        .filter(|(label, _)| label != &NAME && label != &ISA)
        .collect();
    let has_content = !all_edges.is_empty() || new_edge_label.is_some();
    let is_collapsed = editor.tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));

    let child = match display_label {
        Some(label) => D::Text(label, TextStyle::Literal),
        None => D::Identicon(*uuid),
    };

    let mut header_items: Vec<D> = vec![
        D::NodeHeader { child: Box::new(child) },
    ];
    if has_content {
        header_items.push(D::CollapseToggle { collapsed: is_collapsed });
    }

    let mut block_items = vec![D::Line(header_items)];

    if !is_collapsed && has_content {
        let child_ancestors = ancestors.update(id.clone());
        let ctx = RenderCtx { editor, path, id: &id, ancestors: &child_ancestors };

        let mut field_items: Vec<D> = Vec::new();
        for (label, _value) in &all_edges {
            field_items.push(D::Line(vec![
                D::FieldLabel { label_id: label.clone() },
                D::Text(":".into(), TextStyle::Punctuation),
                ctx.descend(label),
            ]));
        }

        if let Some(ref new_label) = new_edge_label {
            let placeholder_path = path.child(new_label.clone());
            let closure_path = placeholder_path.clone();
            field_items.push(D::Line(vec![
                D::FieldLabel { label_id: new_label.clone() },
                D::Text(":".into(), TextStyle::Punctuation),
                D::Placeholder {
                    active: placeholder_active(editor, &placeholder_path, move |w, value| {
                        w.set_edge(&closure_path, value);
                    }),
                },
            ]));
        }

        block_items.push(D::Indent(Box::new(D::Block(field_items))));
    }

    D::Block(block_items)
}

struct ListElement {
    tail_path: Path,
    head_path: Path,
    head_value: Option<Id>,
}

fn flatten_list(editor: &Editor, path: &Path, node: &Id) -> Option<(Vec<ListElement>, Path)> {
    let mut elements = Vec::new();
    let mut current_path = path.clone();
    let mut current_id = node;
    let mut seen = im::HashSet::new();

    while editor.is_cons(current_id) {
        if seen.contains(current_id) {
            return None;
        }
        seen = seen.update(current_id.clone());

        let head_value = editor.doc.gid.get(current_id, &HEAD).cloned();
        let head_path = current_path.child(HEAD.clone());
        let tail_path = current_path.child(TAIL.clone());
        elements.push(ListElement {
            tail_path: tail_path.clone(),
            head_path,
            head_value,
        });

        let tail_value = editor.doc.gid.get(current_id, &TAIL)?;
        current_path = tail_path;
        current_id = tail_value;
    }

    if editor.is_empty(current_id) {
        Some((elements, current_path))
    } else {
        None
    }
}

fn is_list_insertion_selected(editor: &Editor, path: &Path, elements: &[ListElement]) -> Option<usize> {
    let selected_path = editor.selection.as_ref().and_then(|s| s.edge_path())?;

    if selected_path == path && !elements.is_empty() {
        Some(0)
    } else {
        elements.iter()
            .position(|elem| selected_path == &elem.tail_path)
            .map(|i| i + 1)
    }
}

fn render_list(
    editor: &Editor,
    path: &Path,
    id: &Id,
    ancestors: im::HashSet<Id>,
) -> D {
    match flatten_list(editor, path, id) {
        Some((elements, _empty_path)) => {
            let insertion_idx = is_list_insertion_selected(editor, path, &elements);
            let list_ancestors = ancestors.update(id.clone());

            let mut items: Vec<D> = Vec::new();

            for (i, elem) in elements.iter().enumerate() {
                if insertion_idx == Some(i) {
                    let insert_path = if i == 0 { path } else { &elements[i-1].tail_path };
                    items.push(list_placeholder(editor, insert_path));
                }

                let head_d = match &elem.head_value {
                    Some(head) => {
                        render_id(editor, &elem.head_path, head, list_ancestors.clone())
                    }
                    None => {
                        let selected = editor.selection.as_ref()
                            .and_then(|s| s.edge_path()) == Some(&elem.head_path);
                        if selected {
                            list_placeholder(editor, &elem.head_path)
                        } else {
                            D::Text("_".into(), TextStyle::Punctuation)
                        }
                    }
                };
                items.push(head_d);
            }

            if let Some(last) = elements.last() {
                if insertion_idx == Some(elements.len()) {
                    items.push(list_placeholder(editor, &last.tail_path));
                }
            }

            if items.is_empty() {
                if insertion_idx == Some(0) {
                    items.push(list_placeholder(editor, path));
                }
            }

            D::List {
                opening: "[".into(),
                closing: "]".into(),
                separator: "".into(),
                items,
                vertical: true,
            }
        }
        None => {
            if let Id::Uuid(uuid) = id {
                render_uuid(editor, path, uuid, ancestors)
            } else {
                D::Text("?".into(), TextStyle::Literal)
            }
        }
    }
}

fn list_placeholder(editor: &Editor, insert_path: &Path) -> D {
    let current_value = editor.doc.node(insert_path);
    let commit_path = insert_path.clone();
    D::Placeholder {
        active: placeholder_active(editor, insert_path, move |w, head_value| {
            if let Some(ref current_value) = current_value {
                let new_cons = Id::new_uuid();
                w.set_edge(&commit_path, new_cons.clone());
                w.set_edge(&commit_path.child(ISA.clone()), CONS_TYPE.clone());
                w.set_edge(&commit_path.child(HEAD.clone()), head_value);
                w.set_edge(&commit_path.child(TAIL.clone()), current_value.clone());
            }
        }),
    }
}

// Domain projections

type Projection = fn(&RenderCtx) -> Option<D>;

const PROJECTIONS: &[Projection] = &[render_field, render_apply];

fn try_domain_render(ctx: &RenderCtx) -> Option<D> {
    PROJECTIONS.iter().find_map(|p| p(ctx))
}

fn render_field(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.doc.gid;
    Field::try_wrap(gid, ctx.id)?;
    let field = Field::wrap(ctx.id.clone());
    let name = field.name(gid).unwrap_or_else(|| "?".into());

    let mut items = vec![
        D::Text("field".into(), TextStyle::Keyword),
        D::Text(format!("\"{}\"", name), TextStyle::Literal),
    ];

    if field.type_(gid).is_some() {
        items.push(D::Text(":".into(), TextStyle::Punctuation));
        items.push(ctx.descend(&TYPE_));
    }

    Some(D::Line(items))
}

fn render_apply(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.doc.gid;
    let apply = Apply::try_wrap(gid, ctx.id)?;

    let base_name = apply.base(gid)
        .and_then(|b| ctx.editor.name_of(b.id()))
        .unwrap_or_else(|| "?".into());

    let mut items = vec![D::Text(base_name, TextStyle::TypeRef)];

    if let Some(args_id) = gid.get(ctx.id, &ARGS) {
        let arg_items: Vec<D> = ListIter::new(gid, Some(args_id))
            .map(|arg_id| render_type_inline(ctx.editor, arg_id))
            .collect();
        items.push(D::List {
            opening: "<".into(),
            closing: ">".into(),
            separator: ", ".into(),
            items: arg_items,
            vertical: false,
        });
    }

    Some(D::Line(items))
}

fn render_type_inline(editor: &Editor, node: &Id) -> D {
    let gid = &editor.doc.gid;
    if let Some(apply) = Apply::try_wrap(gid, node) {
        let base_name = apply.base(gid)
            .and_then(|b| editor.name_of(b.id()))
            .unwrap_or_else(|| "?".into());

        let mut items = vec![D::Text(base_name, TextStyle::TypeRef)];

        if let Some(args_id) = gid.get(node, &ARGS) {
            let arg_items: Vec<D> = ListIter::new(gid, Some(args_id))
                .map(|arg_id| render_type_inline(editor, arg_id))
                .collect();
            items.push(D::List {
                opening: "<".into(),
                closing: ">".into(),
                separator: ", ".into(),
                items: arg_items,
                vertical: false,
            });
        }

        D::Line(items)
    } else {
        let name = editor.name_of(node).unwrap_or_else(|| "?".into());
        D::Text(name, TextStyle::TypeRef)
    }
}

// Helpers

fn editing_state(editor: &Editor, path: &Path) -> Option<String> {
    match &editor.selection {
        Some(Selection::Edge(sel_path, EdgeState::EditingLeaf(text))) if sel_path == path => {
            Some(text.clone())
        }
        _ => None,
    }
}

fn placeholder_active(
    editor: &Editor,
    path: &Path,
    on_commit: impl Fn(&mut crate::document::EditorWriter, Id) + 'static,
) -> Option<ActivePlaceholder> {
    match &editor.selection {
        Some(Selection::Edge(sel_path, EdgeState::Cursor(ps))) if sel_path == path => {
            Some(ActivePlaceholder {
                state: ps.clone(),
                on_commit: Box::new(on_commit),
            })
        }
        _ => None,
    }
}
