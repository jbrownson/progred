use crate::editor::Editor;
use crate::generated::{display_label, name_of};
use crate::generated::semantics::{Apply, Field, Forall, Record, Sum, Type, ARGS, BASE, BODY, CONS_TYPE, EMPTY_TYPE, FIELDS, HEAD, ISA, NAME, PARAMS, TAIL, TYPE_, VARIANTS};
use crate::graph::{Gid, Id};
use crate::path::Path;
use crate::selection::{EdgeState, Selection};

use crate::d::{D, TextStyle};

pub fn render(editor: &Editor, path: &Path, id: &Id) -> D {
    render_id(editor, path, id, im::HashSet::new())
}

fn render_id(editor: &Editor, path: &Path, id: &Id, ancestors: im::HashSet<Id>) -> D {
    let child = render_id_inner(editor, path, id, ancestors);
    D::Descend { path: path.clone(), selection: Selection::edge(path.clone()), child: Box::new(child) }
}

fn render_id_inner(editor: &Editor, path: &Path, id: &Id, ancestors: im::HashSet<Id>) -> D {
    match id {
        Id::Uuid(_) if editor.lib().get(id, &ISA).is_some_and(|t| t == &CONS_TYPE || t == &EMPTY_TYPE) => {
            render_list(editor, path, id, ancestors)
        }
        Id::Uuid(uuid) if !ancestors.contains(id) => {
            let ctx = RenderCtx { editor, path, id, ancestors: &ancestors };
            try_domain_render(&ctx)
                .unwrap_or_else(|| render_uuid(editor, path, uuid, ancestors))
        }
        Id::Uuid(uuid) => {
            render_uuid(editor, path, uuid, ancestors)
        }
        Id::String(s) => D::StringEditor {
            value: s.clone(),
        },
        Id::Number(n) => D::NumberEditor {
            value: n.0,
            number_text: number_text(editor, path),
        },
    }
}

struct RenderCtx<'a> {
    editor: &'a Editor,
    path: &'a Path,
    id: &'a Id,
    ancestors: &'a im::HashSet<Id>,
}

struct ListStyle {
    opening: &'static str,
    closing: &'static str,
    separator: &'static str,
    vertical: bool,
}

const BRACKET_LIST: ListStyle = ListStyle { opening: "[", closing: "]", separator: "", vertical: true };
const ANGLE_LIST: ListStyle = ListStyle { opening: "<", closing: ">", separator: ", ", vertical: false };

impl<'a> RenderCtx<'a> {
    fn is_collapsed(&self) -> bool {
        self.editor.tree.is_collapsed(self.path)
            .unwrap_or(self.ancestors.contains(self.id))
    }

    fn descend(&self, label: &Id) -> D {
        self.descend_with(label, None)
    }

    fn descend_with(&self, label: &Id, render: Option<fn(&RenderCtx) -> Option<D>>) -> D {
        let child_path = self.path.child(label.clone());
        let selection = Selection::edge(child_path.clone());

        match self.editor.lib().get(self.id, label) {
            Some(child_id) => {
                let child_ancestors = self.ancestors.update(self.id.clone());
                let child = render.and_then(|f| {
                    let child_ctx = RenderCtx { editor: self.editor, path: &child_path, id: child_id, ancestors: &child_ancestors };
                    f(&child_ctx)
                }).unwrap_or_else(|| {
                    render_id_inner(self.editor, &child_path, child_id, child_ancestors)
                });
                D::Descend { path: child_path, selection, child: Box::new(child) }
            }
            None => {
                let commit_path = child_path.clone();
                D::Descend {
                    path: child_path,
                    selection,
                    child: Box::new(D::Placeholder {
                        on_commit: Box::new(move |w: &mut Editor, value| {
                            w.doc.set_edge(&commit_path, value);
                        }),
                    }),
                }
            }
        }
    }

    fn descend_list(&self, label: &Id, style: &ListStyle, item_render: Option<fn(&RenderCtx) -> Option<D>>) -> D {
        let child_path = self.path.child(label.clone());
        let selection = Selection::edge(child_path.clone());

        match self.editor.lib().get(self.id, label) {
            Some(child_id) => {
                let child_ancestors = self.ancestors.update(self.id.clone());
                let child = render_list_styled(self.editor, &child_path, child_id, child_ancestors, style, item_render);
                D::Descend { path: child_path, selection, child: Box::new(child) }
            }
            None => {
                let commit_path = child_path.clone();
                D::Descend {
                    path: child_path,
                    selection,
                    child: Box::new(D::Placeholder {
                        on_commit: Box::new(move |w: &mut Editor, value| {
                            w.doc.set_edge(&commit_path, value);
                        }),
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
    let lib = editor.lib();
    let edges = lib.edges(&id);
    let display_label = display_label(&lib, &id);
    let new_edge_label = editor.selection.as_ref()
        .and_then(|s| s.path())
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

    let header_items: Vec<D> = [D::NodeHeader { child: Box::new(child) }].into_iter()
        .chain(has_content.then(|| D::CollapseToggle { collapsed: is_collapsed }))
        .collect();

    let content = (!is_collapsed && has_content).then(|| {
        let child_ancestors = ancestors.update(id.clone());
        let ctx = RenderCtx { editor, path, id: &id, ancestors: &child_ancestors };

        let field_items: Vec<D> = all_edges.iter()
            .map(|(label, _)| D::Line(vec![
                D::FieldLabel { label_id: label.clone() },
                D::Text(":".into(), TextStyle::Punctuation),
                ctx.descend(label),
            ]))
            .chain(new_edge_label.as_ref().map(|new_label| {
                let placeholder_path = path.child(new_label.clone());
                let closure_path = placeholder_path.clone();
                D::Line(vec![
                    D::FieldLabel { label_id: new_label.clone() },
                    D::Text(":".into(), TextStyle::Punctuation),
                    D::Descend {
                        path: placeholder_path.clone(),
                        selection: Selection::edge(placeholder_path),
                        child: Box::new(D::Placeholder {
                            on_commit: Box::new(move |w: &mut Editor, value| {
                                w.doc.set_edge(&closure_path, value);
                            }),
                        }),
                    },
                ])
            }))
            .collect();

        D::Indent(Box::new(D::Block(field_items)))
    });

    let block_items: Vec<D> = [D::Line(header_items)].into_iter()
        .chain(content)
        .collect();

    D::Block(block_items)
}

struct ListElement {
    head_path: Path,
    head_value: Option<Id>,
    cons_id: Id,
}

fn flatten_list(editor: &Editor, path: &Path, node: &Id) -> Option<(Vec<ListElement>, Path)> {
    let lib = editor.lib();
    let mut elements = Vec::new();
    let mut current_path = path.clone();
    let mut current_id = node;
    let mut seen = im::HashSet::new();

    while lib.get(current_id, &ISA) == Some(&CONS_TYPE) {
        if seen.contains(current_id) {
            return None;
        }
        seen = seen.update(current_id.clone());

        let head_value = lib.get(current_id, &HEAD).cloned();
        let head_path = current_path.child(HEAD.clone());
        elements.push(ListElement {
            head_path,
            head_value,
            cons_id: current_id.clone(),
        });

        let tail_path = current_path.child(TAIL.clone());
        let tail_value = lib.get(current_id, &TAIL)?;
        current_path = tail_path;
        current_id = tail_value;
    }

    if lib.get(current_id, &ISA) == Some(&EMPTY_TYPE) {
        Some((elements, current_path))
    } else {
        None
    }
}


fn render_list(
    editor: &Editor,
    path: &Path,
    id: &Id,
    ancestors: im::HashSet<Id>,
) -> D {
    render_list_styled(editor, path, id, ancestors, &BRACKET_LIST, None)
}

fn render_list_styled(
    editor: &Editor,
    path: &Path,
    id: &Id,
    ancestors: im::HashSet<Id>,
    style: &ListStyle,
    item_render: Option<fn(&RenderCtx) -> Option<D>>,
) -> D {
    match flatten_list(editor, path, id) {
        Some((elements, _empty_path)) => {
            let list_ancestors = ancestors.update(id.clone());

            let list_elements: Vec<D> = elements.iter().map(|elem| {
                let selection = Selection::ListElement {
                    path: elem.head_path.clone(),
                    cons_id: elem.cons_id.clone(),
                    edge_state: EdgeState::default(),
                };
                match &elem.head_value {
                    Some(head) => {
                        let child = item_render.and_then(|f| {
                            let child_ctx = RenderCtx { editor, path: &elem.head_path, id: head, ancestors: &list_ancestors };
                            f(&child_ctx)
                        }).unwrap_or_else(|| render_id_inner(editor, &elem.head_path, head, list_ancestors.clone()));
                        D::Descend { path: elem.head_path.clone(), selection, child: Box::new(child) }
                    }
                    None => {
                        let commit_path = elem.head_path.clone();
                        D::Descend {
                            path: elem.head_path.clone(),
                            selection,
                            child: Box::new(D::Placeholder {
                                on_commit: Box::new(move |w: &mut Editor, value| {
                                    w.doc.set_edge(&commit_path, value);
                                }),
                            }),
                        }
                    }
                }
            }).collect();

            if style.vertical {
                D::VerticalList { elements: list_elements }
            } else {
                D::HorizontalList {
                    opening: style.opening.into(),
                    closing: style.closing.into(),
                    separator: style.separator.into(),
                    elements: list_elements,
                }
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

type Projection = fn(&RenderCtx) -> Option<D>;

const PROJECTIONS: &[Projection] = &[render_field, render_apply, render_type, render_record, render_sum, render_forall];

fn try_domain_render(ctx: &RenderCtx) -> Option<D> {
    PROJECTIONS.iter().find_map(|p| p(ctx))
}

fn render_ref(ctx: &RenderCtx) -> Option<D> {
    Some(match ctx.id {
        Id::Uuid(uuid) => {
            let inner = match name_of(&ctx.editor.lib(), ctx.id) {
                Some(name) => D::Text(name, TextStyle::TypeRef),
                None => D::Identicon(*uuid),
            };
            D::NodeHeader { child: Box::new(inner) }
        }
        Id::String(s) => D::StringEditor { value: s.clone() },
        Id::Number(n) => D::NumberEditor { value: n.0, number_text: None },
    })
}

fn render_type_expr(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    if Type::try_wrap(gid, ctx.id).is_some() {
        render_ref(ctx)
    } else {
        None // fall through to default projection
    }
}

fn render_field(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Field::try_wrap(gid, ctx.id)?;
    Some(D::Line(vec![
        D::NodeHeader { child: Box::new(D::Text("field".into(), TextStyle::Keyword)) },
        ctx.descend(&NAME),
        D::Text(":".into(), TextStyle::Punctuation),
        ctx.descend_with(&TYPE_, Some(render_type_expr)),
    ]))
}

fn render_apply(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Apply::try_wrap(gid, ctx.id)?;

    Some(D::Line(vec![
        D::NodeHeader { child: Box::new(ctx.descend_with(&BASE, Some(render_ref))) },
        ctx.descend_list(&ARGS, &ANGLE_LIST, Some(render_type_expr)),
    ]))
}

fn render_type(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Type::try_wrap(gid, ctx.id)?;
    Some(D::Line(vec![
        D::NodeHeader { child: Box::new(D::Text("type".into(), TextStyle::Keyword)) },
        ctx.descend(&NAME),
        D::Text("=".into(), TextStyle::Punctuation),
        ctx.descend(&BODY),
    ]))
}

fn render_record(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Record::try_wrap(gid, ctx.id)?;
    let collapsed = ctx.is_collapsed();
    let items: Vec<D> = [D::Line(vec![
        D::NodeHeader { child: Box::new(D::Text("record".into(), TextStyle::Keyword)) },
        D::CollapseToggle { collapsed },
    ])].into_iter()
        .chain((!collapsed).then(|| D::Indent(Box::new(ctx.descend(&FIELDS)))))
        .collect();
    Some(D::Block(items))
}

fn render_sum(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Sum::try_wrap(gid, ctx.id)?;
    let collapsed = ctx.is_collapsed();
    let items: Vec<D> = [D::Line(vec![
        D::NodeHeader { child: Box::new(D::Text("sum".into(), TextStyle::Keyword)) },
        D::CollapseToggle { collapsed },
    ])].into_iter()
        .chain((!collapsed).then(|| D::Indent(Box::new(ctx.descend(&VARIANTS)))))
        .collect();
    Some(D::Block(items))
}

fn render_forall(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Forall::try_wrap(gid, ctx.id)?;
    let collapsed = ctx.is_collapsed();
    let items: Vec<D> = [
        D::NodeHeader { child: Box::new(D::Text("forall".into(), TextStyle::Keyword)) },
        ctx.descend_list(&PARAMS, &ANGLE_LIST, Some(render_param)),
        D::CollapseToggle { collapsed },
    ].into_iter()
        .chain(if collapsed { vec![] } else { vec![
            D::Text(".".into(), TextStyle::Punctuation),
            ctx.descend(&BODY),
        ]})
        .collect();
    Some(D::Line(items))
}

fn render_param(ctx: &RenderCtx) -> Option<D> {
    let gid = &ctx.editor.lib();
    Type::try_wrap(gid, ctx.id)?;
    Some(ctx.descend(&NAME))
}

fn number_text(editor: &Editor, path: &Path) -> Option<String> {
    match &editor.selection {
        Some(Selection::Edge(sel_path, es)) if sel_path == path => es.number_text.clone(),
        Some(Selection::ListElement { path: sel_path, edge_state, .. }) if sel_path == path => edge_state.number_text.clone(),
        _ => None,
    }
}
