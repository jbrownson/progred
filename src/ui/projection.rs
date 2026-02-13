use crate::document::{Editor, EditorWriter};
use crate::graph::{EdgeState, Id, Path, Selection};
use eframe::egui::{self, pos2, Color32, CornerRadius, Response, Sense, Ui, Vec2};
use ordered_float::OrderedFloat;

use crate::d::{D, NodeDisplay, TextStyle};

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

fn text_color(style: &TextStyle) -> Color32 {
    match style {
        TextStyle::Default => Color32::from_gray(60),
        TextStyle::Keyword => Color32::from_rgb(150, 100, 150),
        TextStyle::TypeRef => Color32::from_rgb(80, 130, 180),
        TextStyle::Punctuation => Color32::from_gray(120),
        TextStyle::Literal => Color32::from_gray(60),
    }
}

fn text_rich(s: &str, style: &TextStyle) -> egui::RichText {
    let rt = egui::RichText::new(s).color(text_color(style));
    match style {
        TextStyle::Keyword => rt.italics(),
        _ => rt,
    }
}

pub fn project(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, path: &Path, mode: &InteractionMode) {
    if let Some(id) = editor.doc.node(path) {
        let d = crate::render::render(editor, path, &id);
        render_d(ui, editor, w, &d, mode);
    }
}

fn render_d(ui: &mut Ui, editor: &Editor, w: &mut EditorWriter, d: &D, mode: &InteractionMode) {
    match d {
        D::Block(children) => {
            ui.vertical(|ui| {
                for child in children {
                    render_d(ui, editor, w, child, mode);
                }
            });
        }
        D::Line(children) => {
            ui.horizontal(|ui| {
                for child in children {
                    render_d(ui, editor, w, child, mode);
                }
            });
        }
        D::Indent(child) => {
            ui.indent("edges", |ui| {
                render_d(ui, editor, w, child, mode);
            });
        }
        D::Spacing(n) => {
            ui.add_space(*n);
        }
        D::Text(s, style) => {
            ui.label(text_rich(s, style));
        }
        D::LabelArrow => {
            label_arrow(ui);
        }
        D::NodeHeader { path, id, display } => {
            render_node_header(ui, editor, w, path, id, display, mode);
        }
        D::FieldLabel { entity_path, label_id } => {
            render_field_label(ui, editor, w, entity_path, label_id, mode);
        }
        D::CollapseToggle { path, collapsed } => {
            if collapse_toggle(ui, *collapsed).clicked() {
                w.set_collapsed(path, !collapsed);
            }
        }
        D::StringEditor { path, value } => {
            render_string_editor(ui, editor, w, path, value);
        }
        D::NumberEditor { path, value, editing } => {
            render_number_editor(ui, editor, w, path, *value, editing.as_deref());
        }
        D::Placeholder { active } => {
            render_placeholder(ui, w, active);
        }
        D::List { opening, closing, separator, items } => {
            ui.label(text_rich(opening, &TextStyle::Punctuation));
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    ui.label(text_rich(separator, &TextStyle::Punctuation));
                }
                render_d(ui, editor, w, item, mode);
            }
            ui.label(text_rich(closing, &TextStyle::Punctuation));
        }
    }
}

fn render_node_header(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    id: &Id,
    display: &NodeDisplay,
    mode: &InteractionMode,
) {
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && editor.selected_node_id().as_ref() == Some(id);

    let (style, hovered) = if primary || secondary {
        let s = selection_style(primary, secondary);
        (s, s)
    } else {
        mode_style(mode)
    };
    if clickable(ui, |ui| {
        match display {
            NodeDisplay::Named(label) => ui.label(egui::RichText::new(label).color(Color32::from_gray(60))),
            NodeDisplay::Identicon(uuid) => identicon(ui, 18.0, uuid),
        }
    }, style, hovered).clicked() {
        handle_pick(w, mode, id.clone(), path);
    }
}

fn render_field_label(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    entity_path: &Path,
    label_id: &Id,
    mode: &InteractionMode,
) {
    let label_color = Color32::from_gray(120);
    let secondary = editor.selected_node_id().as_ref() == Some(label_id);
    let (style, hovered) = if secondary {
        let s = selection_style(false, true);
        (s, s)
    } else if matches!(mode, InteractionMode::Normal) {
        ((None, None), (None, None))
    } else {
        mode_style(mode)
    };
    ui.push_id(label_id, |ui| {
        if clickable(ui, |ui| match label_id {
            Id::Uuid(uuid) => match editor.name_of(label_id) {
                Some(name) => ui.label(egui::RichText::new(name).color(label_color).italics()),
                None => identicon(ui, 12.0, uuid),
            },
            Id::String(s) => ui.label(egui::RichText::new(s.to_string()).color(label_color).italics()),
            Id::Number(n) => ui.label(egui::RichText::new(n.to_string()).color(label_color).italics()),
        }, style, hovered).clicked()
            && !matches!(mode, InteractionMode::Normal)
        {
            handle_pick(w, mode, label_id.clone(), entity_path);
        }
    });
}

fn render_string_editor(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    value: &str,
) {
    let id = Id::String(value.to_string());
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && editor.selected_node_id().as_ref() == Some(&id);

    let leaf_edit_text = match &editor.selection {
        Some(Selection::Edge(_, EdgeState::EditingLeaf(t))) if primary => Some(t),
        _ => None,
    };
    let is_editing = leaf_edit_text.is_some();
    let mut text = if is_editing {
        leaf_edit_text.cloned().unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
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
        w.start_leaf_edit(value.to_string());
    }

    if response.lost_focus() {
        if let Some(final_text) = w.stop_leaf_edit() {
            w.set_edge(path, Id::String(final_text));
        }
    }

    if is_editing && leaf_edit_text.is_some_and(|t| t != &text) {
        w.set_edge(path, Id::String(text.clone()));
        w.update_leaf_edit_text(text);
    }
}

fn render_number_editor(
    ui: &mut Ui,
    editor: &Editor,
    w: &mut EditorWriter,
    path: &Path,
    value: f64,
    _editing: Option<&str>,
) {
    let id = Id::Number(OrderedFloat(value));
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && editor.selected_node_id().as_ref() == Some(&id);

    let model_text = value.to_string();
    let leaf_edit_text = match &editor.selection {
        Some(Selection::Edge(_, EdgeState::EditingLeaf(t))) if primary => Some(t),
        _ => None,
    };
    let is_editing = leaf_edit_text.is_some();
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
        w.start_leaf_edit(model_text.clone());
    }

    if response.lost_focus() {
        if let Some(final_text) = w.stop_leaf_edit() {
            if let Ok(n) = final_text.parse::<f64>() {
                w.set_edge(path, Id::Number(OrderedFloat(n)));
            }
        }
    }

    if is_editing && leaf_edit_text.is_some_and(|t| t != &text) {
        w.update_leaf_edit_text(text);
    }
}

fn render_placeholder(
    ui: &mut Ui,
    w: &mut EditorWriter,
    active: &Option<crate::d::ActivePlaceholder>,
) {
    if let Some(active) = active {
        let mut ps = active.state.clone();
        match super::placeholder::render(ui, &mut ps) {
            PlaceholderResult::Commit(value) => {
                (active.on_commit)(w, value);
                w.select(None);
            }
            PlaceholderResult::Dismiss => w.select(None),
            PlaceholderResult::Active => w.set_placeholder_state(ps),
        }
    }
}
