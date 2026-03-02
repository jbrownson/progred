use progred_core::graph::{Id, PlaceholderState};
use eframe::egui::{self, Ui};
use progred_core::ordered_float::OrderedFloat;

pub struct PlaceholderResult {
    pub text_changed: Option<String>,
    pub selection_moved: Option<usize>,
    pub outcome: PlaceholderOutcome,
}

pub enum PlaceholderOutcome {
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

pub fn render(ui: &mut Ui, ps: &PlaceholderState) -> PlaceholderResult {
    let entries = build_entries(&ps.text);
    let selected_index = ps.selected_index.min(entries.len() - 1);

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

    let text_changed = if ps.text != text {
        Some(text)
    } else {
        None
    };

    let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let arrow_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
    let arrow_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));

    let selection_moved = if arrow_down {
        Some((selected_index + 1).min(entries.len() - 1))
    } else if arrow_up && selected_index > 0 {
        Some(selected_index - 1)
    } else {
        None
    };

    let commit_index = popup_commit.or_else(|| {
        enter.then_some(selected_index)
    });

    let outcome = match commit_index {
        Some(i) => PlaceholderOutcome::Commit(entries.into_iter().nth(i).unwrap().value.commit()),
        None if escape || text_response.lost_focus() => PlaceholderOutcome::Dismiss,
        None => PlaceholderOutcome::Active,
    };

    PlaceholderResult {
        text_changed,
        selection_moved,
        outcome,
    }
}
