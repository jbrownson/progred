use crate::graph::{Id, PlaceholderState};
use eframe::egui::{self, Ui};
use ordered_float::OrderedFloat;

pub enum PlaceholderResult {
    Active,
    Commit(Id),
    Dismiss,
}

enum PlaceholderValue {
    Id(Id),
    NewUuid,
}

impl PlaceholderValue {
    fn commit(self) -> Id {
        match self {
            PlaceholderValue::Id(id) => id,
            PlaceholderValue::NewUuid => Id::new_uuid(),
        }
    }
}

struct PlaceholderEntry {
    value: PlaceholderValue,
    display: String,
}

// TODO: look into existing fuzzy finder work (fzf, nucleo, etc.) for filtering entries
fn build_entries(filter: &str) -> Vec<PlaceholderEntry> {
    let trimmed = filter.trim_start_matches('"').trim_end_matches('"');
    let string_entry = (!trimmed.is_empty()).then(|| PlaceholderEntry {
        value: PlaceholderValue::Id(Id::String(trimmed.to_string())),
        display: format!("\"{}\"", trimmed),
    });
    let number_entry = filter.parse::<f64>().ok().map(|n| PlaceholderEntry {
        value: PlaceholderValue::Id(Id::Number(OrderedFloat(n))),
        display: n.to_string(),
    });
    string_entry.into_iter()
        .chain(number_entry)
        .chain(std::iter::once(PlaceholderEntry {
            value: PlaceholderValue::NewUuid,
            display: "New node".to_string(),
        }))
        .collect()
}

pub fn render(ui: &mut Ui, ps: &mut PlaceholderState) -> PlaceholderResult {
    let entries = build_entries(&ps.text);
    ps.selected_index = ps.selected_index.min(entries.len() - 1);
    let selected_index = ps.selected_index;

    let mut text = ps.text.clone();
    let text_id = ui.id().with("placeholder_input");

    let text_response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(text_id)
            .desired_width(150.0)
            .hint_text("search...")
    );
    text_response.request_focus();

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
                            clicked = Some(i);
                        }
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
        ps.selected_index = (ps.selected_index + 1).min(entries.len() - 1);
    }
    if arrow_up && ps.selected_index > 0 {
        ps.selected_index -= 1;
    }

    let commit_index = popup_commit.or_else(|| {
        enter.then_some(selected_index)
    });

    match commit_index {
        Some(i) => PlaceholderResult::Commit(entries.into_iter().nth(i).unwrap().value.commit()),
        None if escape || text_response.lost_focus() => PlaceholderResult::Dismiss,
        None => PlaceholderResult::Active,
    }
}
