use progred_core::d::{D, DEvent, TextStyle};
use progred_core::editor::{Editor, InteractionMode};
use progred_core::generated::name_of;
use progred_core::graph::{Id, Path};
use eframe::egui::{self, pos2, Color32, CornerRadius, Response, Sense, Ui, Vec2};

use super::colors;
use super::identicon;
use super::placeholder::PlaceholderOutcome;

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

pub fn compute_interaction_mode(modifiers: egui::Modifiers, editor: &Editor) -> InteractionMode {
    let selected_path = editor.selection.as_ref().and_then(|s| s.edge_path()).cloned();
    if modifiers.alt {
        match selected_path {
            Some(path) => InteractionMode::Assign(path),
            _ => InteractionMode::Normal,
        }
    } else if modifiers.ctrl {
        match selected_path {
            Some(ref path) if matches!(editor.doc.node(path), Some(Id::Uuid(_))) => {
                InteractionMode::SelectUnder(path.clone())
            }
            _ => InteractionMode::Normal,
        }
    } else {
        InteractionMode::Normal
    }
}

pub struct DContext {
    pub path: Path,
}

pub fn render_d<'a>(ui: &mut Ui, editor: &Editor, d: &'a D, mode: &InteractionMode, ctx: &DContext, events: &mut Vec<DEvent<'a>>) {
    match d {
        D::Block(children) => {
            ui.vertical(|ui| {
                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(2.0);
                    }
                    render_d(ui, editor, child, mode, ctx, events);
                }
            });
        }
        D::Line(children) => {
            ui.horizontal(|ui| {
                for child in children {
                    render_d(ui, editor, child, mode, ctx, events);
                }
            });
        }
        D::Indent(child) => {
            ui.indent("edges", |ui| {
                render_d(ui, editor, child, mode, ctx, events);
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
            render_d(ui, editor, child, mode, &child_ctx, events);
        }
        D::NodeHeader { child } => {
            render_node_header(ui, editor, child, mode, ctx, events);
        }
        D::FieldLabel { label_id } => {
            render_field_label(ui, editor, &ctx.path, label_id, mode, events);
        }
        D::CollapseToggle { collapsed } => {
            if collapse_toggle(ui, *collapsed).clicked() {
                events.push(DEvent::ClickedCollapseToggle(ctx.path.clone()));
            }
        }
        D::StringEditor { value } => {
            render_string_editor(ui, editor, &ctx.path, value, events);
        }
        D::NumberEditor { value, number_text } => {
            render_number_editor(ui, editor, &ctx.path, *value, number_text.as_deref(), events);
        }
        D::Placeholder { on_commit } => {
            render_placeholder(ui, editor, &ctx.path, on_commit, events);
        }
        D::List { opening, closing, separator, items, vertical } => {
            if *vertical && !items.is_empty() {
                ui.vertical(|ui| {
                    for item in items {
                        ui.horizontal(|ui| {
                            ui.label(text_rich("\u{2022}", &TextStyle::Punctuation));
                            render_d(ui, editor, item, mode, ctx, events);
                        });
                    }
                });
            } else {
                ui.label(text_rich(opening, &TextStyle::Punctuation));
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        ui.label(text_rich(separator, &TextStyle::Punctuation));
                    }
                    render_d(ui, editor, item, mode, ctx, events);
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

fn render_node_header<'a>(
    ui: &mut Ui,
    editor: &Editor,
    child: &'a D,
    mode: &InteractionMode,
    ctx: &DContext,
    events: &mut Vec<DEvent<'a>>,
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
        render_d(ui, editor, child, mode, ctx, events);
        ui.interact(ui.min_rect(), ui.id(), Sense::hover())
    }, style, hovered).clicked() {
        if let Some(id) = id {
            events.push(DEvent::ClickedNode { path: ctx.path.clone(), id });
        }
    }
}

fn render_field_label(
    ui: &mut Ui,
    editor: &Editor,
    entity_path: &Path,
    label_id: &Id,
    mode: &InteractionMode,
    events: &mut Vec<DEvent<'_>>,
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
            events.push(DEvent::ClickedFieldLabel { entity_path: entity_path.clone(), label_id: label_id.clone() });
        }
    });
}

fn render_string_editor(
    ui: &mut Ui,
    editor: &Editor,
    path: &Path,
    value: &str,
    events: &mut Vec<DEvent<'_>>,
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
        events.push(DEvent::ClickedStringEditor(path.clone()));
    }

    if text != value {
        events.push(DEvent::StringEditorStringChanged { path: path.clone(), text });
    }
}

fn render_number_editor(
    ui: &mut Ui,
    editor: &Editor,
    path: &Path,
    value: f64,
    number_text: Option<&str>,
    events: &mut Vec<DEvent<'_>>,
) {
    let id = Id::Number(progred_core::ordered_float::OrderedFloat(value));
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
        events.push(DEvent::ClickedNumberEditor(path.clone()));
    }

    if is_editing && text != display_text {
        events.push(DEvent::NumberEditorTextChanged { path: path.clone(), text });
    }
}

fn render_placeholder<'a>(
    ui: &mut Ui,
    editor: &Editor,
    path: &Path,
    on_commit: &'a dyn Fn(&mut Editor, Id),
    events: &mut Vec<DEvent<'a>>,
) {
    let ps = match &editor.selection {
        Some(progred_core::graph::Selection::Edge(sel_path, es)) if sel_path == path => &es.placeholder,
        _ => return,
    };
    let result = super::placeholder::render(ui, ps);
    match result.outcome {
        PlaceholderOutcome::Commit(value) => {
            events.push(DEvent::PlaceholderCommitted { on_commit, value });
        }
        PlaceholderOutcome::Dismiss => {
            events.push(DEvent::PlaceholderDismissed);
        }
        PlaceholderOutcome::Active => {
            if let Some(text) = result.text_changed {
                events.push(DEvent::PlaceholderTextChanged(text));
            }
            if let Some(index) = result.selection_moved {
                events.push(DEvent::PlaceholderSelectionMoved(index));
            }
        }
    }
}
