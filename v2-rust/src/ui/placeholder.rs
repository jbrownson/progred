use crate::graph::{Id, PlaceholderState};
use eframe::egui::{self, Ui};
use ordered_float::OrderedFloat;

pub enum PlaceholderResult {
    Active,
    Commit(Id),
    Dismiss,
}

struct PlaceholderEntry {
    id: Id,
    display: String,
}

fn build_entries(filter: &str) -> Vec<PlaceholderEntry> {
    let trimmed = filter.trim_start_matches('"').trim_end_matches('"');
    let string_entry = (!trimmed.is_empty()).then(|| PlaceholderEntry {
        id: Id::String(trimmed.to_string()),
        display: format!("\"{}\"", trimmed),
    });
    let number_entry = filter.parse::<f64>().ok().map(|n| PlaceholderEntry {
        id: Id::Number(OrderedFloat(n)),
        display: n.to_string(),
    });
    string_entry.into_iter().chain(number_entry).collect()
}

pub fn render(ui: &mut Ui, ps: &mut PlaceholderState) -> PlaceholderResult {
    let entries = build_entries(&ps.text);
    let total = entries.len() + 1; // +1 for "New node"
    ps.selected_index = ps.selected_index.min(total - 1);
    let selected_index = ps.selected_index;

    let mut text = ps.text.clone();
    let text_id = ui.id().with("placeholder_input");

    let text_response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(text_id)
            .desired_width(150.0)
            .hint_text("search...")
    );
    ui.memory_mut(|mem| mem.request_focus(text_id));

    let popup_commit = {
        let mut clicked = None;
        egui::Popup::from_response(&text_response)
            .id(ui.id().with("placeholder_popup"))
            .open(true)
            .width(text_response.rect.width())
            .show(|ui| {
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        if ui.selectable_label(i == selected_index, &entry.display).clicked() {
                            clicked = Some(entry.id.clone());
                        }
                    }
                    if ui.selectable_label(entries.len() == selected_index, "New node").clicked() {
                        clicked = Some(Id::new_uuid());
                    }
                });
            });
        clicked
    };

    if ps.text != text {
        ps.text = text;
        ps.selected_index = 0;
    }

    let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let arrow_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
    let arrow_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));

    if arrow_down {
        ps.selected_index = (ps.selected_index + 1).min(total - 1);
    }
    if arrow_up && ps.selected_index > 0 {
        ps.selected_index -= 1;
    }

    let commit = popup_commit.or_else(|| {
        enter.then(|| {
            entries.get(selected_index)
                .map(|e| e.id.clone())
                .unwrap_or_else(Id::new_uuid)
        })
    });

    match commit {
        Some(id) => PlaceholderResult::Commit(id),
        None if escape => PlaceholderResult::Dismiss,
        None => PlaceholderResult::Active,
    }
}
