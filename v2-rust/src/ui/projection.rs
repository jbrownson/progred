use crate::document::{Editor, EditorWriter};
use crate::generated::semantics::{CONS_TYPE, HEAD, ISA, NAME, TAIL};
use crate::graph::{Gid, Id, Path, Selection};
use eframe::egui::{self, pos2, Color32, CornerRadius, Response, Sense, Ui, Vec2};
use im::HashSet;
use ordered_float::OrderedFloat;

use super::colors;
use super::identicon;
use super::placeholder::PlaceholderResult;

pub mod layout {
    pub const SELECTION_PADDING: f32 = 2.0;

    const CARET_SIZE: f32 = 10.0;
    pub const CARET_WIDTH: f32 = CARET_SIZE * 0.6;
    pub const CARET_HALF_HEIGHT: f32 = CARET_SIZE * 0.4;
    const CARET_OVERLAP: f32 = 1.0;
    pub const CARET_INSET: f32 = CARET_WIDTH - CARET_OVERLAP;

    const fn max_f32(a: f32, b: f32) -> f32 {
        if a > b { a } else { b }
    }

    pub const TREE_MARGIN: f32 = max_f32(CARET_INSET, SELECTION_PADDING) + 2.0;
}

pub enum InteractionMode {
    Normal,
    SelectUnder(Path),
    Assign(Path),
}

type HighlightStyle = (Option<Color32>, Option<eframe::epaint::Stroke>);

fn paint_highlight(
    ui: &Ui,
    rect: eframe::egui::Rect,
    bg_idx: eframe::egui::layers::ShapeIdx,
    border_idx: eframe::egui::layers::ShapeIdx,
    style: HighlightStyle,
) {
    let (bg, border) = style;
    let rounding = CornerRadius::same(3);
    if let Some(bg) = bg {
        ui.painter().set(bg_idx, eframe::epaint::Shape::rect_filled(rect, rounding, bg));
    }
    if let Some(border) = border {
        ui.painter().set(border_idx, eframe::epaint::Shape::rect_stroke(rect, rounding, border, eframe::epaint::StrokeKind::Middle));
    }
}

fn clickable(
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> Response,
    style: HighlightStyle,
    hovered_style: HighlightStyle,
) -> Response {
    let id = ui.next_auto_id();
    let bg_idx = ui.painter().add(eframe::epaint::Shape::Noop);
    let border_idx = ui.painter().add(eframe::epaint::Shape::Noop);

    let inner = add_contents(ui);
    let rect = inner.rect.expand(layout::SELECTION_PADDING);
    let response = ui.interact(rect, id, Sense::click());

    paint_highlight(ui, rect, bg_idx, border_idx, if response.hovered() { hovered_style } else { style });
    response
}

fn highlighted(
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> Response,
    style: HighlightStyle,
) -> Response {
    let bg_idx = ui.painter().add(eframe::epaint::Shape::Noop);
    let border_idx = ui.painter().add(eframe::epaint::Shape::Noop);

    let response = add_contents(ui);
    let rect = response.rect.expand(layout::SELECTION_PADDING);

    paint_highlight(ui, rect, bg_idx, border_idx, style);
    response
}

fn selection_style(selected: bool, secondary: bool) -> HighlightStyle {
    if selected {
        (Some(colors::SELECTION.gamma_multiply(0.3)), Some(eframe::epaint::Stroke::new(1.5, colors::SELECTION)))
    } else if secondary {
        (Some(colors::SELECTION.gamma_multiply(0.15)), Some(eframe::epaint::Stroke::new(1.0, colors::SELECTION.gamma_multiply(0.4))))
    } else {
        (None, None)
    }
}

fn mode_style(mode: &InteractionMode) -> (HighlightStyle, HighlightStyle) {
    match mode {
        InteractionMode::Assign(_) => (
            (Some(colors::ASSIGN.gamma_multiply(0.2)), Some(eframe::epaint::Stroke::new(1.0, colors::ASSIGN.gamma_multiply(0.6)))),
            (Some(colors::ASSIGN.gamma_multiply(0.4)), Some(eframe::epaint::Stroke::new(1.0, colors::ASSIGN.gamma_multiply(0.6)))),
        ),
        InteractionMode::SelectUnder(_) => (
            (Some(colors::SELECT_UNDER.gamma_multiply(0.2)), Some(eframe::epaint::Stroke::new(1.0, colors::SELECT_UNDER.gamma_multiply(0.6)))),
            (Some(colors::SELECT_UNDER.gamma_multiply(0.4)), Some(eframe::epaint::Stroke::new(1.0, colors::SELECT_UNDER.gamma_multiply(0.6)))),
        ),
        InteractionMode::Normal => (
            (None, None),
            (Some(Color32::from_gray(200).gamma_multiply(0.5)), None),
        ),
    }
}

pub fn insertion_point(ui: &mut Ui) -> Response {
    let width = ui.available_width().min(200.0);

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 0.0), Sense::hover());

    let hit_rect = eframe::egui::Rect::from_center_size(rect.center(), Vec2::new(width, 10.0));
    let response = ui.interact(hit_rect, ui.next_auto_id(), Sense::click());

    if response.hovered() {
        let center_y = rect.center().y;
        let left_x = rect.min.x - layout::CARET_INSET;

        ui.painter().add(eframe::epaint::Shape::convex_polygon(
            vec![
                pos2(left_x, center_y - layout::CARET_HALF_HEIGHT),
                pos2(left_x + layout::CARET_WIDTH, center_y),
                pos2(left_x, center_y + layout::CARET_HALF_HEIGHT),
            ],
            Color32::from_gray(150),
            eframe::epaint::Stroke::NONE,
        ));
    }

    response
}

fn render_label(ui: &mut Ui, editor: &Editor, id: &Id, secondary: bool, mode: &InteractionMode) -> Response {
    let label_color = Color32::from_gray(120);
    let (style, hovered) = if secondary {
        let s = selection_style(false, true);
        (s, s)
    } else if matches!(mode, InteractionMode::Normal) {
        ((None, None), (None, None))
    } else {
        mode_style(mode)
    };
    clickable(ui, |ui| match id {
        Id::Uuid(uuid) => match editor.name_of(id) {
            Some(name) => ui.label(eframe::egui::RichText::new(name).color(label_color).italics()),
            None => identicon(ui, 12.0, uuid),
        },
        Id::String(s) => ui.label(eframe::egui::RichText::new(s.to_string()).color(label_color).italics()),
        Id::Number(n) => ui.label(eframe::egui::RichText::new(n.to_string()).color(label_color).italics()),
    }, style, hovered)
}

fn label_arrow(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 10.0), Sense::hover());

    if ui.is_rect_visible(rect) {
        let color = Color32::from_gray(160);
        let stroke = eframe::epaint::Stroke::new(1.0, color);
        let center_y = rect.center().y;
        let left = rect.min.x + 1.0;
        let right = rect.max.x - 2.0;

        ui.painter().line_segment([pos2(left, center_y), pos2(right, center_y)], stroke);

        let arrow_size = 3.0;
        ui.painter().line_segment([pos2(right - arrow_size, center_y - arrow_size), pos2(right, center_y)], stroke);
        ui.painter().line_segment([pos2(right - arrow_size, center_y + arrow_size), pos2(right, center_y)], stroke);
    }
}

fn collapse_toggle(ui: &mut Ui, collapsed: bool) -> Response {
    let size = 12.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());

    if ui.is_rect_visible(rect) {
        let color = if response.hovered() { Color32::from_gray(80) } else { Color32::from_gray(150) };

        if response.hovered() {
            ui.painter().rect_filled(rect.expand(1.0), CornerRadius::same(2), Color32::from_gray(220));
        }
        let center = rect.center();
        let half = size * 0.3;

        let points = if collapsed {
            vec![
                pos2(center.x - half * 0.5, center.y - half),
                pos2(center.x - half * 0.5, center.y + half),
                pos2(center.x + half, center.y),
            ]
        } else {
            vec![
                pos2(center.x - half, center.y - half * 0.5),
                pos2(center.x + half, center.y - half * 0.5),
                pos2(center.x, center.y + half),
            ]
        };

        ui.painter().add(eframe::epaint::Shape::convex_polygon(points, color, eframe::epaint::Stroke::NONE));
    }

    response
}

pub fn project(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, mode: &InteractionMode) {
    if let Some(id) = editor.doc.node(path) {
        project_id(ui, editor, w, path, &id, HashSet::new(), mode);
    }
}

fn project_id(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
    mode: &InteractionMode,
) {
    match id {
        Id::Uuid(_) if editor.is_list(id) => project_list(ui, editor, w, path, id, ancestors, mode),
        Id::Uuid(uuid) => project_uuid(ui, editor, w, path, uuid, ancestors, mode),
        Id::String(_) | Id::Number(_) => project_leaf(ui, editor, w, path, id),
    }
}

fn project_leaf(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, id: &Id) {
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary
        && editor.selected_node_id().as_ref() == Some(id);

    let model_text = match id {
        Id::String(s) => s.clone(),
        Id::Number(n) => n.to_string(),
        Id::Uuid(_) => unreachable!(),
    };

    let leaf_edit_text = editor.selection.as_ref().and_then(|s| s.leaf_edit_text.as_ref());
    let is_editing = primary && editor.editing_leaf;
    let mut text = if is_editing {
        leaf_edit_text.cloned().unwrap_or_else(|| model_text.clone())
    } else {
        model_text.clone()
    };

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.clone(), font_id, Color32::BLACK);
    let text_width = galley.rect.width();
    let desired_width = text_width.max(20.0);

    let stable_id = egui::Id::new(path);
    let response = highlighted(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(stable_id)
                .desired_width(desired_width)
                .frame(false)
        )
    }, selection_style(primary, secondary));

    if response.gained_focus() {
        w.select(Some(Selection::edge(path.clone())));
        w.set_editing_leaf(true);
        w.set_leaf_edit_text(Some(model_text.clone()));
    }

    if response.lost_focus() {
        let new_id = leaf_edit_text.and_then(|edit_text| match id {
            Id::String(_) => Some(Id::String(edit_text.clone())),
            Id::Number(_) => edit_text.parse::<f64>().ok().map(|n| Id::Number(OrderedFloat(n))),
            Id::Uuid(_) => unreachable!(),
        });
        if let Some(new_id) = new_id {
            w.set_edge(path, new_id);
        }
        w.set_editing_leaf(false);
    }

    if is_editing && Some(&text) != leaf_edit_text {
        w.set_leaf_edit_text(Some(text.clone()));
        if matches!(id, Id::String(_)) {
            w.set_edge(path, Id::String(text));
        }
    }
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
    let mut seen = HashSet::new();

    while editor.is_cons(current_id) {
        if seen.contains(current_id) {
            return None;
        }
        seen.insert(current_id.clone());

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

fn list_punct(ui: &mut Ui, w: &mut EditorWriter, text: &str, path: &Path, color: Color32) {
    if ui.add(egui::Button::new(
        eframe::egui::RichText::new(text).color(color)
    ).frame(false).sense(Sense::click())).clicked() {
        w.select(Some(Selection::edge(path.clone())));
    }
}

fn project_list(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    id: &Id,
    ancestors: HashSet<Id>,
    mode: &InteractionMode,
) {
    match flatten_list(editor, path, id) {
        Some((elements, _empty_path)) => {
            let insertion_idx = is_list_insertion_selected(editor, path, &elements);
            let list_ancestors = ancestors.update(id.clone());

            ui.vertical(|ui| {
                if elements.is_empty() {
                    let punct_color = Color32::from_gray(120);
                    ui.horizontal(|ui| {
                        list_punct(ui, w, "[]", path, punct_color);
                        if insertion_idx == Some(0) {
                            render_list_placeholder(ui, editor, w, path);
                        }
                    });
                } else {
                    for (i, elem) in elements.iter().enumerate() {
                        if insertion_idx == Some(i) {
                            let insert_path = if i == 0 { path } else { &elements[i-1].tail_path };
                            ui.horizontal(|ui| {
                                render_list_placeholder(ui, editor, w, insert_path);
                            });
                        }

                        ui.horizontal(|ui| {
                            ui.label(eframe::egui::RichText::new("•").color(Color32::from_gray(150)));
                            match &elem.head_value {
                                Some(head) => {
                                    project_id(ui, editor, w, &elem.head_path, head, list_ancestors.clone(), mode);
                                }
                                None => {
                                    let selected = editor.selection.as_ref()
                                        .and_then(|s| s.edge_path()) == Some(&elem.head_path);
                                    if selected {
                                        render_list_placeholder(ui, editor, w, &elem.head_path);
                                    } else {
                                        list_punct(ui, w, "_", &elem.head_path, Color32::from_gray(150));
                                    }
                                }
                            }
                        });
                    }

                    if let Some(last) = elements.last()
                        && insertion_idx == Some(elements.len())
                    {
                        ui.horizontal(|ui| {
                            render_list_placeholder(ui, editor, w, &last.tail_path);
                        });
                    }
                }
            });
        }
        None => {
            if let Id::Uuid(uuid) = id {
                project_uuid(ui, editor, w, path, uuid, ancestors, mode);
            }
        }
    }
}

fn render_list_placeholder(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, insert_path: &Path) {
    if let Some(ref sel) = editor.selection {
        let mut ps = sel.placeholder.clone();
        match super::placeholder::render(ui, &mut ps) {
            PlaceholderResult::Commit(value) => {
                do_list_insert(w, editor, insert_path, value);
                w.select(None);
            }
            PlaceholderResult::Dismiss => w.select(None),
            PlaceholderResult::Active => w.set_placeholder_state(ps),
        }
    }
}

fn do_list_insert(w: &mut EditorWriter, editor: &Editor, insert_path: &Path, head_value: Id) {
    if let Some(current_value) = editor.doc.node(insert_path) {
        let new_cons = Id::new_uuid();
        w.set_edge(insert_path, new_cons.clone());
        w.set_edge(&insert_path.child(ISA.clone()), CONS_TYPE.clone());
        w.set_edge(&insert_path.child(HEAD.clone()), head_value);
        w.set_edge(&insert_path.child(TAIL.clone()), current_value);
    }
}

fn handle_pick(w: &mut EditorWriter, mode: &InteractionMode, value: Id, path: &Path) {
    match mode {
        InteractionMode::Assign(target) => {
            w.set_edge(target, value);
            w.select(None);
        }
        InteractionMode::SelectUnder(source) => {
            w.select(Some(Selection::edge(source.child(value))));
        }
        InteractionMode::Normal => {
            w.select(Some(Selection::edge(path.clone())));
        }
    }
}

fn project_uuid(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    uuid: &uuid::Uuid,
    ancestors: HashSet<Id>,
    mode: &InteractionMode,
) {
    let id = Id::Uuid(*uuid);

    // Try domain-specific projections first
    if super::domain_projections::try_domain_projection(ui, editor, w, path, &id, ancestors.clone(), mode) {
        return;
    }
    let edges = editor.doc.gid.edges(&id);
    let display_label = editor.display_label(&id);
    let new_edge_label = editor.selection.as_ref()
        .and_then(|s| s.edge_path())
        .and_then(|sel| sel.pop())
        .filter(|(parent, _)| parent == path)
        .map(|(_, label)| label)
        .filter(|label| !edges.map(|e| e.contains_key(label)).unwrap_or(false));
    let all_edges: Vec<(Id, Id)> = edges.into_iter()
        .flat_map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())))
        .filter(|(label, _)| label != &NAME && label != &ISA)
        .collect();
    let has_content = !all_edges.is_empty() || new_edge_label.is_some();
    let is_collapsed = editor.tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));
    let selected_node = editor.selected_node_id();
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && selected_node.as_ref() == Some(&id);

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let (style, hovered) = if primary || secondary {
                let s = selection_style(primary, secondary);
                (s, s)
            } else {
                mode_style(mode)
            };
            if clickable(ui, |ui| {
                match display_label {
                    Some(ref label) => ui.label(eframe::egui::RichText::new(label).color(Color32::from_gray(60))),
                    None => identicon(ui, 18.0, uuid),
                }
            }, style, hovered).clicked() {
                handle_pick(w, mode, Id::Uuid(*uuid), path);
            }

            if has_content && collapse_toggle(ui, is_collapsed).clicked() {
                w.set_collapsed(path, !is_collapsed);
            }
        });

        if !is_collapsed && has_content {
            let child_ancestors = ancestors.update(id.clone());
            ui.add_space(2.0);
            ui.indent("edges", |ui| {
                for (label, value) in &all_edges {
                    let label_secondary = selected_node.as_ref() == Some(label);
                    ui.push_id(label, |ui| {
                        ui.horizontal(|ui| {
                            if render_label(ui, editor, label, label_secondary, mode).clicked()
                                && !matches!(mode, InteractionMode::Normal)
                            {
                                handle_pick(w, mode, label.clone(), path);
                            }

                            label_arrow(ui);
                            project_id(ui, editor, w, &path.child(label.clone()), value, child_ancestors.clone(), mode);
                        });
                        ui.add_space(2.0);
                    });
                }
                if let Some(ref new_label) = new_edge_label {
                    ui.horizontal(|ui| {
                        render_label(ui, editor, new_label, false, &InteractionMode::Normal);
                        label_arrow(ui);
                        if let Some(ref sel) = editor.selection {
                            let mut ps = sel.placeholder.clone();
                            match super::placeholder::render(ui, &mut ps) {
                                PlaceholderResult::Commit(value) => {
                                    w.set_edge(&path.child(new_label.clone()), value);
                                    w.select(None);
                                }
                                PlaceholderResult::Dismiss => w.select(None),
                                PlaceholderResult::Active => w.set_placeholder_state(ps),
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
            });
        }
    });
}
