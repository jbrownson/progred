use progred_core::d::PlaceholderCommit;
use progred_core::editor::Editor;
use progred_core::generated::{name_of, semantics::{ISA, Type}};
use progred_core::graph::{Gid, Id};
use progred_core::selection::PlaceholderState;
use eframe::egui::{self, Color32, Ui};
use progred_core::ordered_float::OrderedFloat;
use std::collections::HashMap;

pub struct PlaceholderResult {
    pub text_changed: Option<String>,
    pub selection_moved: Option<usize>,
    pub outcome: PlaceholderOutcome,
}

pub enum PlaceholderOutcome {
    Active,
    Commit(PlaceholderCommit),
    Dismiss,
}

#[derive(Clone)]
enum PlaceholderValue {
    Ref(Id),
    Literal(Id),
    NewTyped { isa: Id },
    NewUuid,
}

impl PlaceholderValue {
    fn commit(self) -> PlaceholderCommit {
        match self {
            PlaceholderValue::Ref(id) | PlaceholderValue::Literal(id) => PlaceholderCommit::Existing(id),
            PlaceholderValue::NewTyped { isa } => PlaceholderCommit::NewNode { isa },
            PlaceholderValue::NewUuid => PlaceholderCommit::Existing(Id::new_uuid()),
        }
    }
}

#[derive(Clone)]
struct PlaceholderEntry {
    value: PlaceholderValue,
    display: String,
    disambiguation: Option<String>,
    magic: bool,
}

struct NamedThing {
    name: String,
    id: Id,
}

fn named_things(editor: &Editor) -> Vec<NamedThing> {
    let mut by_id: HashMap<Id, NamedThing> = HashMap::new();
    let lib = editor.lib();

    for uuid in editor.doc.gid.entities() {
        let id = Id::Uuid(*uuid);
        if let Some(name) = name_of(&lib, &id) {
            by_id.insert(id.clone(), NamedThing { name, id });
        }
    }

    for uuid in editor.semantics.gid.entities() {
        let id = Id::Uuid(*uuid);
        if let Some(name) = name_of(&lib, &id) {
            by_id.entry(id.clone()).or_insert(NamedThing { name, id });
        }
    }

    let mut things: Vec<NamedThing> = by_id.into_values().collect();
    things.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    things
}

fn disambiguation(gid: &impl Gid, id: &Id) -> Option<String> {
    let isa = gid.get(id, &ISA)?;
    name_of(gid, isa)
}

fn is_type_node(gid: &impl Gid, id: &Id) -> bool {
    Type::try_wrap(gid, id).is_some()
}

fn filter_entries(entries: &[PlaceholderEntry], needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return (0..entries.len()).map(|i| (i, 0)).collect();
    }

    let mut result: Vec<(usize, usize)> = Vec::new();
    let mut accepted: Vec<bool> = vec![false; entries.len()];

    let needle_lower = needle.to_lowercase();
    let display_lowers: Vec<String> = entries.iter().map(|e| e.display.to_lowercase()).collect();

    let tiers: Vec<Box<dyn Fn(usize) -> bool>> = vec![
        Box::new(|i| entries[i].display.starts_with(needle)),
        Box::new(|i| entries[i].display.contains(needle)),
        Box::new(|i| display_lowers[i].starts_with(&needle_lower)),
        Box::new(|i| display_lowers[i].contains(&needle_lower)),
        Box::new(|i| fuzzy_match(&entries[i].display, needle)),
        Box::new(|i| fuzzy_match(&display_lowers[i], &needle_lower)),
    ];

    for (tier, predicate) in tiers.iter().enumerate() {
        for i in 0..entries.len() {
            if !accepted[i] && predicate(i) {
                accepted[i] = true;
                result.push((i, tier + 1));
            }
        }
    }

    result.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| {
            let pct_a = needle.len() as f64 / entries[a.0].display.len().max(1) as f64;
            let pct_b = needle.len() as f64 / entries[b.0].display.len().max(1) as f64;
            pct_b.partial_cmp(&pct_a).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    result
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

fn build_entries(editor: &Editor, filter: &str) -> Vec<PlaceholderEntry> {
    let things = named_things(editor);
    let lib = editor.lib();

    // Data entries (references to existing named nodes)
    let mut all_entries: Vec<PlaceholderEntry> = things.iter().map(|t| PlaceholderEntry {
        value: PlaceholderValue::Ref(t.id.clone()),
        display: t.name.clone(),
        disambiguation: disambiguation(&lib, &t.id),
        magic: false,
    }).collect();

    // "New X" constructor entries for type nodes
    for t in &things {
        if is_type_node(&lib, &t.id) {
            all_entries.push(PlaceholderEntry {
                value: PlaceholderValue::NewTyped { isa: t.id.clone() },
                display: format!("new {}", t.name),
                disambiguation: None,
                magic: false,
            });
        }
    }

    // Magic entries (string/number literals)
    let trimmed = filter.trim_start_matches('"').trim_end_matches('"');
    if !trimmed.is_empty() {
        all_entries.push(PlaceholderEntry {
            value: PlaceholderValue::Literal(Id::String(trimmed.to_string())),
            display: format!("\"{}\"", trimmed),
            disambiguation: None,
            magic: true,
        });
    }
    if let Ok(n) = filter.parse::<f64>() {
        all_entries.push(PlaceholderEntry {
            value: PlaceholderValue::Literal(Id::Number(OrderedFloat(n))),
            display: n.to_string(),
            disambiguation: None,
            magic: true,
        });
    }

    // Plain "New node" entry
    all_entries.push(PlaceholderEntry {
        value: PlaceholderValue::NewUuid,
        display: "New node".to_string(),
        disambiguation: None,
        magic: false,
    });

    // Filter
    let filtered_indices = filter_entries(&all_entries, filter);

    // Sort: non-magic before magic, then by filter tier
    let mut sorted: Vec<(usize, usize)> = filtered_indices;
    sorted.sort_by(|a, b| {
        let a_magic = all_entries[a.0].magic;
        let b_magic = all_entries[b.0].magic;
        a_magic.cmp(&b_magic).then_with(|| a.1.cmp(&b.1))
    });

    sorted.into_iter().map(|(i, _)| all_entries[i].clone()).collect()
}

pub fn render(ui: &mut Ui, editor: &Editor, ps: &PlaceholderState) -> PlaceholderResult {
    let entries = build_entries(editor, &ps.text);
    let selected_index = if entries.is_empty() { 0 } else { ps.selected_index.min(entries.len() - 1) };

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
                        let mut job = egui::text::LayoutJob::default();
                        job.append(&entry.display, 0.0, egui::TextFormat::simple(
                            egui::TextStyle::Body.resolve(ui.style()),
                            ui.visuals().text_color(),
                        ));
                        if let Some(dis) = &entry.disambiguation {
                            job.append(&format!(" ({dis})"), 0.0, egui::TextFormat::simple(
                                egui::TextStyle::Body.resolve(ui.style()),
                                Color32::from_gray(160),
                            ));
                        }
                        if ui.selectable_label(i == selected_index, job).clicked() {
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

    let max_index = if entries.is_empty() { 0 } else { entries.len() - 1 };
    let selection_moved = if arrow_down {
        Some((selected_index + 1).min(max_index))
    } else if arrow_up && selected_index > 0 {
        Some(selected_index - 1)
    } else {
        None
    };

    let commit_index = popup_commit.or_else(|| {
        enter.then_some(selected_index)
    });

    let outcome = match commit_index {
        Some(i) if i < entries.len() => {
            PlaceholderOutcome::Commit(entries.into_iter().nth(i).unwrap().value.commit())
        }
        _ if escape || text_response.lost_focus() => PlaceholderOutcome::Dismiss,
        _ => PlaceholderOutcome::Active,
    };

    PlaceholderResult {
        text_changed,
        selection_moved,
        outcome,
    }
}
