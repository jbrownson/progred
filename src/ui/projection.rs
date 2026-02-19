use crate::editor::Editor;
use crate::generated::name_of;
use crate::graph::{EdgeState, Id, Path, Selection};
use eframe::egui::{self, pos2, Color32, CornerRadius, Response, Sense, Ui, Vec2};
use ordered_float::OrderedFloat;

use crate::d::{D, TextStyle};

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
    const fn max_f32(a: f32, b: f32) -> f32 { if a > b { a } else { b } }
    pub const TREE_MARGIN: f32 = max_f32(CARET_INSET, SELECTION_PADDING) + 2.0;
}

pub enum InteractionMode {
    Normal,
    SelectUnder(Path),
    Assign(Path),
}

pub struct DContext {
    pub path: Path,
}

pub fn render_d(ui: &mut Ui, editor: &mut Editor, d: &D, mode: &InteractionMode, ctx: &DContext) {
    match d {
        D::Block(children) => {
            ui.vertical(|ui| {
                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(2.0);
                    }
                    render_d(ui, editor, child, mode, ctx);
                }
            });
        }
        D::Line(children) => {
            ui.horizontal(|ui| {
                for child in children {
                    render_d(ui, editor, child, mode, ctx);
                }
            });
        }
        D::Indent(child) => {
            ui.indent("edges", |ui| {
                render_d(ui, editor, child, mode, ctx);
            });
        }
        D::Text(s, style) => {
            ui.label(text_rich(s, style));
        }
        D::Identicon(uuid) => {
            identicon(ui, 18.0, uuid);
        }
        D::Descend { path, child } => {
            let child_ctx = DContext { path: path.clone() };
            render_d(ui, editor, child, mode, &child_ctx);
        }
        D::NodeHeader { child } => {
            render_node_header(ui, editor, child, mode, ctx);
        }
        D::FieldLabel { label_id } => {
            render_field_label(ui, editor, &ctx.path, label_id, mode);
        }
        D::CollapseToggle { collapsed } => {
            if collapse_toggle(ui, *collapsed).clicked() {
                editor.tree.set_collapsed(&ctx.path, !collapsed);
            }
        }
        D::StringEditor { value } => {
            render_string_editor(ui, editor, &ctx.path, value);
        }
        D::NumberEditor { value, number_text } => {
            render_number_editor(ui, editor, &ctx.path, *value, number_text.as_deref());
        }
        D::Placeholder { on_commit } => {
            render_placeholder(ui, editor, &ctx.path, on_commit);
        }
        D::List { opening, closing, separator, items, vertical } => {
            if *vertical && !items.is_empty() {
                ui.vertical(|ui| {
                    for item in items {
                        ui.horizontal(|ui| {
                            ui.label(text_rich("\u{2022}", &TextStyle::Punctuation));
                            render_d(ui, editor, item, mode, ctx);
                        });
                    }
                });
            } else {
                ui.label(text_rich(opening, &TextStyle::Punctuation));
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        ui.label(text_rich(separator, &TextStyle::Punctuation));
                    }
                    render_d(ui, editor, item, mode, ctx);
                }
                ui.label(text_rich(closing, &TextStyle::Punctuation));
            }
        }
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

// --- Private helpers ---

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

fn handle_pick(editor: &mut Editor, mode: &InteractionMode, value: Id, path: &Path) {
    match mode {
        InteractionMode::Assign(target) => {
            editor.doc.set_edge(target, value);
            editor.selection = None;
        }
        InteractionMode::SelectUnder(source) => {
            editor.selection = Some(Selection::edge(source.child(value)));
        }
        InteractionMode::Normal => {
            editor.selection = Some(Selection::edge(path.clone()));
        }
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

fn render_node_header(
    ui: &mut Ui,
    editor: &mut Editor,
    child: &D,
    mode: &InteractionMode,
    ctx: &DContext,
) {
    let id = editor.doc.node(&ctx.path);
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(&ctx.path);
    let secondary = !primary && id.is_some() && editor.selected_node_id().as_ref() == id.as_ref();

    let (style, hovered) = if primary || secondary {
        let s = selection_style(primary, secondary);
        (s, s)
    } else {
        mode_style(mode)
    };
    if clickable(ui, |ui| {
        render_d(ui, editor, child, mode, ctx);
        ui.interact(ui.min_rect(), ui.id(), Sense::hover())
    }, style, hovered).clicked() {
        if let Some(id) = id {
            handle_pick(editor, mode, id, &ctx.path);
        }
    }
}

fn render_field_label(
    ui: &mut Ui,
    editor: &mut Editor,
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
            Id::Uuid(uuid) => match name_of(&editor.doc.gid, label_id) {
                Some(name) => ui.label(egui::RichText::new(name).color(label_color).italics()),
                None => identicon(ui, 12.0, uuid),
            },
            Id::String(s) => ui.label(egui::RichText::new(s.to_string()).color(label_color).italics()),
            Id::Number(n) => ui.label(egui::RichText::new(n.to_string()).color(label_color).italics()),
        }, style, hovered).clicked()
            && !matches!(mode, InteractionMode::Normal)
        {
            handle_pick(editor, mode, label_id.clone(), entity_path);
        }
    });
}

fn render_string_editor(
    ui: &mut Ui,
    editor: &mut Editor,
    path: &Path,
    value: &str,
) {
    let id = Id::String(value.to_string());
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && editor.selected_node_id().as_ref() == Some(&id);

    let mut text = value.to_string();

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
        editor.selection = Some(Selection::edge(path.clone()));
    }

    if text != value {
        editor.doc.set_edge(path, Id::String(text));
    }
}

fn render_number_editor(
    ui: &mut Ui,
    editor: &mut Editor,
    path: &Path,
    value: f64,
    number_text: Option<&str>,
) {
    let id = Id::Number(OrderedFloat(value));
    let primary = editor.selection.as_ref().and_then(|s| s.edge_path()) == Some(path);
    let secondary = !primary && editor.selected_node_id().as_ref() == Some(&id);

    let is_editing = number_text.is_some();
    let display_text = number_text.map_or_else(|| value.to_string(), |t| t.to_string());
    let mut text = display_text.clone();

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
        let mut es = EdgeState::default();
        es.number_text = Some(value.to_string());
        editor.selection = Some(Selection::Edge(path.clone(), es));
    }

    if response.lost_focus() {
        if let Some(text) = number_text {
            if let Ok(n) = text.parse::<f64>() {
                editor.doc.set_edge(path, Id::Number(OrderedFloat(n)));
            }
            if let Some(Selection::Edge(_, ref mut es)) = editor.selection {
                es.number_text = None;
            }
        }
    }

    if is_editing && text != display_text {
        if let Some(Selection::Edge(_, ref mut es)) = editor.selection {
            es.number_text = Some(text);
        }
    }
}

fn render_placeholder(
    ui: &mut Ui,
    editor: &mut Editor,
    path: &Path,
    on_commit: &dyn Fn(&mut Editor, Id),
) {
    let ps = match &editor.selection {
        Some(Selection::Edge(sel_path, es)) if sel_path == path => es.placeholder.clone(),
        _ => return,
    };
    let mut ps = ps;
    match super::placeholder::render(ui, &mut ps) {
        PlaceholderResult::Commit(value) => {
            on_commit(editor, value);
            editor.selection = None;
        }
        PlaceholderResult::Dismiss => editor.selection = None,
        PlaceholderResult::Active => {
            if let Some(Selection::Edge(_, ref mut es)) = editor.selection {
                es.placeholder = ps;
            }
        }
    }
}
