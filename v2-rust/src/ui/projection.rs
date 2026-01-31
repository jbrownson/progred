use crate::document::{Editor, EditorWriter};
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

fn render_label(ui: &mut Ui, id: &Id, secondary: bool, mode: &InteractionMode) -> Response {
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
        Id::Uuid(uuid) => identicon(ui, 12.0, uuid),
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
        Id::Uuid(uuid) => project_uuid(ui, editor, w, path, uuid, ancestors, mode),
        Id::String(_) | Id::Number(_) => project_leaf(ui, editor, w, path, id),
    }
}

fn project_leaf(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, id: &Id) {
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary
        && editor.selection.as_ref().and_then(|s| s.selected_node_id(&editor.doc)).as_ref() == Some(id);

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

    let stable_id = ui.id().with("leaf_editor");
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
        if let Some(edit_text) = w.leaf_edit_text() {
            *edit_text = Some(model_text.clone());
        }
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
        if let Some(edit_text) = w.leaf_edit_text() {
            *edit_text = Some(text.clone());
        }
        if matches!(id, Id::String(_)) {
            w.set_edge(path, Id::String(text));
        }
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
    let edges = editor.doc.gid.edges(&id);
    let new_edge_label = editor.selection.as_ref()
        .and_then(|s| s.edge_path())
        .and_then(|sel| sel.pop())
        .filter(|(parent, _)| parent == path)
        .map(|(_, label)| label)
        .filter(|label| !edges.map(|e| e.contains_key(label)).unwrap_or(false));
    let all_edges: Vec<(Id, Id)> = edges.into_iter()
        .flat_map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())))
        .collect();
    let has_content = !all_edges.is_empty() || new_edge_label.is_some();
    let is_collapsed = editor.tree.is_collapsed(path).unwrap_or(ancestors.contains(&id));
    let selected_node = editor.selection.as_ref().and_then(|s| s.selected_node_id(&editor.doc));
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
            if clickable(ui, |ui| identicon(ui, 18.0, uuid), style, hovered).clicked() {
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
                    ui.horizontal(|ui| {
                        if render_label(ui, label, label_secondary, mode).clicked()
                            && !matches!(mode, InteractionMode::Normal)
                        {
                            handle_pick(w, mode, label.clone(), path);
                        }

                        label_arrow(ui);
                        project_id(ui, editor, w, &path.child(label.clone()), value, child_ancestors.clone(), mode);
                    });
                    ui.add_space(2.0);
                }
                if let Some(ref new_label) = new_edge_label {
                    ui.horizontal(|ui| {
                        render_label(ui, new_label, false, &InteractionMode::Normal);
                        label_arrow(ui);
                        if let Some(ps) = w.placeholder_state() {
                            match super::placeholder::render(ui, ps) {
                                PlaceholderResult::Commit(value) => {
                                    w.set_edge(&path.child(new_label.clone()), value);
                                    w.select(None);
                                }
                                PlaceholderResult::Dismiss => w.select(None),
                                PlaceholderResult::Active => {}
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
            });
        }
    });
}
