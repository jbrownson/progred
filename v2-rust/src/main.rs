mod document;
mod graph;
mod ts_runtime;
mod ui;

use document::{Document, Editor, EditorWriter};
use eframe::egui;
use graph::{Id, MutGid, Path, PlaceholderState, RootSlot, Selection, SelectionTarget};
use ui::placeholder::PlaceholderResult;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Progred"),
        ..Default::default()
    };

    eframe::run_native(
        "Progred",
        options,
        Box::new(|_cc| Ok(Box::new(ProgredApp::new()))),
    )
}

struct ProgredApp {
    editor: Editor,
}

impl ProgredApp {
    fn new() -> Self {
        Self { editor: Editor::new() }
    }

    fn new_document(&mut self) {
        self.editor = Editor::new();
    }

    fn save(&mut self) {
        if self.editor.file_path.is_some() {
            self.save_to_path();
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Progred", &["progred"]).save_file() {
            self.editor.file_path = Some(path);
            self.save_to_path();
        }
    }

    fn save_to_path(&self) {
        if let Some(ref path) = self.editor.file_path {
            let root_ids: Vec<_> = self.editor.doc.roots.iter().map(|r| r.node().clone()).collect();
            let json_doc = serde_json::json!({
                "graph": self.editor.doc.gid.to_json(),
                "roots": root_ids,
            });
            if let Ok(json) = serde_json::to_string_pretty(&json_doc) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn window_title(&self) -> String {
        self.editor.file_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map_or_else(|| "Progred".to_string(), |name| format!("{} - Progred", name))
    }

    fn open(&mut self) {
        if let Some((path, gid, root_ids)) = rfd::FileDialog::new()
            .add_filter("Progred", &["progred"])
            .pick_file()
            .and_then(|path| {
                let contents = std::fs::read_to_string(&path).ok()?;
                let json_doc: serde_json::Value = serde_json::from_str(&contents).ok()?;
                let graph_data = serde_json::from_value(json_doc.get("graph")?.clone()).ok()?;
                let gid = MutGid::from_json(graph_data).ok()?;
                let root_ids: Vec<Id> = serde_json::from_value(json_doc.get("roots")?.clone()).ok()?;
                Some((path, gid, root_ids))
            })
        {
            self.editor = Editor {
                doc: Document { gid, roots: root_ids.into_iter().map(RootSlot::new).collect() },
                file_path: Some(path),
                ..Editor::new()
            };
        }
    }

    fn create_test_data() -> (MutGid, Vec<RootSlot>) {
        let mut gid = MutGid::new();

        let field = Id::new_uuid();
        let name = Id::new_uuid();
        let isa = Id::new_uuid();

        gid.set(field.clone(), isa.clone(), field.clone());
        gid.set(field.clone(), name.clone(), Id::String("field".into()));

        gid.set(name.clone(), isa.clone(), field.clone());
        gid.set(name.clone(), name.clone(), Id::String("name".into()));

        gid.set(isa.clone(), isa.clone(), field.clone());
        gid.set(isa.clone(), name.clone(), Id::String("isa".into()));

        let roots = vec![
            RootSlot::new(field),
            RootSlot::new(name),
            RootSlot::new(isa),
        ];

        (gid, roots)
    }

    fn load_test_data(&mut self) {
        let (gid, roots) = Self::create_test_data();
        self.editor = Editor { doc: Document { gid, roots }, ..Editor::new() };
    }

    fn delete_selection(&mut self) {
        if let Some(path) = self.editor.selection.as_ref().and_then(|s| s.edge_path()) {
            self.editor.doc.delete_path(path);
            self.editor.selection = None;
        }
    }

    fn insert_new_node(&mut self) {
        let sel = match &self.editor.selection {
            Some(s) => s,
            None => return,
        };
        match &sel.target {
            SelectionTarget::InsertRoot(index) => {
                let index = (*index).min(self.editor.doc.roots.len());
                self.editor.doc.roots.insert(index, RootSlot::new(Id::new_uuid()));
            }
            SelectionTarget::Edge(path) => {
                self.editor.doc.set_edge(path, Id::new_uuid());
            }
        }
        self.editor.selection = None;
    }
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));

        let placeholder_active = self.editor.selection.as_ref()
            .map_or(false, |s| s.placeholder.is_some());
        ctx.input(|i| {
            if placeholder_active {
                if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S) {
                    self.save_as();
                } else if i.modifiers.command && i.key_pressed(egui::Key::S) {
                    self.save();
                }
            } else {
                if i.key_pressed(egui::Key::Escape) {
                    self.editor.selection = None;
                } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    self.delete_selection();
                } else if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::N) {
                    if self.editor.selection.is_some() { self.insert_new_node(); }
                } else if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S) {
                    self.save_as();
                } else if i.modifiers.command && i.key_pressed(egui::Key::N) {
                    self.new_document();
                } else if i.modifiers.command && i.key_pressed(egui::Key::O) {
                    self.open();
                } else if i.modifiers.command && i.key_pressed(egui::Key::S) {
                    self.save();
                }
            }
        });

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New").shortcut_text("Cmd+N")).clicked() {
                        self.new_document();
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Open...").shortcut_text("Cmd+O")).clicked() {
                        self.open();
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Save").shortcut_text("Cmd+S")).clicked() {
                        self.save();
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Save As...").shortcut_text("Shift+Cmd+S")).clicked() {
                        self.save_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Load Test Data")).clicked() {
                        self.load_test_data();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(
                        self.editor.selection.is_some(),
                        egui::Button::new("New Node").shortcut_text("Shift+Cmd+N"),
                    ).clicked() {
                        self.insert_new_node();
                        ui.close_menu();
                    }
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.edge_path()).is_some(),
                        egui::Button::new("Delete").shortcut_text("Backspace"),
                    ).clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let snapshot = self.editor.clone();
            let mut w = EditorWriter::new(&mut self.editor);
            render_graph(ui, ctx, &snapshot, &mut w);
        });
    }
}

fn render_graph(ui: &mut egui::Ui, ctx: &egui::Context, editor: &Editor, w: &mut EditorWriter) {
    let bg_response = ui.interact(
        ui.max_rect(),
        ui.id().with("background"),
        egui::Sense::click(),
    );

    let modifiers = ctx.input(|i| i.modifiers);
    let mode = if modifiers.alt {
        match editor.selection.as_ref().and_then(|s| s.edge_path()) {
            Some(path) => ui::InteractionMode::Assign(path.clone()),
            _ => ui::InteractionMode::Normal,
        }
    } else if modifiers.ctrl {
        match editor.selection.as_ref().and_then(|s| s.edge_path()) {
            Some(path) if matches!(path.node(&editor.doc.gid), Some(Id::Uuid(_))) => {
                ui::InteractionMode::SelectUnder(path.clone())
            }
            _ => ui::InteractionMode::Normal,
        }
    } else {
        ui::InteractionMode::Normal
    };

    if editor.doc.roots.is_empty() {
        if matches!(&editor.selection,
            Some(Selection { target: SelectionTarget::InsertRoot(0), placeholder: Some(_), .. }))
        {
            if let Some(ps) = w.placeholder_state() {
                match ui::placeholder::render(ui, ps) {
                    PlaceholderResult::Commit(id) => {
                        w.insert_root(0, RootSlot::new(id));
                        w.select(None);
                    }
                    PlaceholderResult::Dismiss => w.select(None),
                    PlaceholderResult::Active => {}
                }
            }
        } else if ui::insertion_point(ui, false).clicked() {
            w.select(Some(Selection {
                target: SelectionTarget::InsertRoot(0),
                placeholder: Some(PlaceholderState::default()),
            }));
        }
    } else {
        for (i, root_slot) in editor.doc.roots.iter().enumerate() {
            let selected = matches!(&editor.selection, Some(Selection { target: SelectionTarget::InsertRoot(idx), .. }) if *idx == i);
            if ui::insertion_point(ui, selected).clicked() {
                w.select(Some(Selection::insert_root(i)));
            }
            ui.push_id(root_slot, |ui| {
                ui::project(ui, editor, w, &Path::new(root_slot.clone()), &mode);
            });
        }
        let last = editor.doc.roots.len();
        let selected = matches!(&editor.selection, Some(Selection { target: SelectionTarget::InsertRoot(idx), .. }) if *idx == last);
        if ui::insertion_point(ui, selected).clicked() {
            w.select(Some(Selection::insert_root(last)));
        }
    }

    if bg_response.clicked() {
        w.select(None);
    }
}
