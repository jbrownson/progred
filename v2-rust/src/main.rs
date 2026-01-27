mod document;
mod graph;
mod ts_runtime;
mod ui;

use document::{Document, Editor};
use eframe::egui;
use graph::{Id, MutGid, Path, RootSlot, Selection};

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
        match &self.editor.file_path {
            Some(path) => {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled");
                format!("{} - Progred", name)
            }
            None => "Progred".to_string(),
        }
    }

    fn open(&mut self) {
        let result = rfd::FileDialog::new()
            .add_filter("Progred", &["progred"])
            .pick_file()
            .and_then(|path| {
                let contents = std::fs::read_to_string(&path).ok()?;
                let json_doc: serde_json::Value = serde_json::from_str(&contents).ok()?;
                let graph_data = serde_json::from_value(json_doc.get("graph")?.clone()).ok()?;
                let gid = MutGid::from_json(graph_data).ok()?;
                let root_ids: Vec<Id> = serde_json::from_value(json_doc.get("roots")?.clone()).ok()?;
                Some((path, gid, root_ids))
            });

        if let Some((path, gid, root_ids)) = result {
            let roots = root_ids.into_iter().map(RootSlot::new).collect();
            self.editor = Editor { doc: Document { gid, roots }, file_path: Some(path), ..Editor::new() };
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

    fn delete_path(&mut self, path: &Path) {
        match path.pop() {
            None => {
                if let Some(idx) = self.editor.doc.roots.iter().position(|r| r == &path.root) {
                    self.editor.doc.roots.remove(idx);
                }
            }
            Some((parent_path, label)) => {
                if let Some(parent_node) = parent_path.node(&self.editor.doc.gid).cloned() {
                    if let Id::Uuid(_) = parent_node {
                        self.editor.doc.gid.delete(&parent_node, &label);
                    }
                }
            }
        }
    }

    fn delete_selection(&mut self) {
        if let Some(Selection::Edge(ref path)) = self.editor.selection.clone() {
            self.delete_path(&path);
            self.editor.selection = None;
        }
    }

    fn insert_new_node(&mut self) {
        match &self.editor.selection {
            Some(Selection::InsertRoot(index)) => {
                let new_id = Id::new_uuid();
                let index = (*index).min(self.editor.doc.roots.len());
                self.editor.doc.roots.insert(index, RootSlot::new(new_id));
                self.editor.selection = None;
            }
            Some(Selection::Edge(path)) => {
                let new_id = Id::new_uuid();
                self.editor.doc.set_edge(&path.clone(), new_id);
                self.editor.selection = None;
            }
            None => {}
        }
    }
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));

        ctx.input(|i| {
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
                    let can_insert = self.editor.selection.is_some();
                    let can_delete = matches!(self.editor.selection, Some(Selection::Edge(_)));

                    if ui.add_enabled(can_insert, egui::Button::new("New Node").shortcut_text("Shift+Cmd+N")).clicked() {
                        self.insert_new_node();
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_delete, egui::Button::new("Delete").shortcut_text("Backspace")).clicked() {
                        self.delete_selection();
                        ui.close_menu();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let bg_response = ui.interact(
                ui.max_rect(),
                ui.id().with("background"),
                egui::Sense::click(),
            );

            let modifiers = ctx.input(|i| i.modifiers);
            let mode = if modifiers.command {
                match &self.editor.selection {
                    Some(Selection::Edge(path)) => ui::InteractionMode::Cmd(path.clone()),
                    _ => ui::InteractionMode::Normal,
                }
            } else if modifiers.shift {
                match &self.editor.selection {
                    Some(Selection::Edge(path)) if matches!(path.node(&self.editor.doc.gid), Some(Id::Uuid(_))) => {
                        ui::InteractionMode::Shift(path.clone())
                    }
                    _ => ui::InteractionMode::Normal,
                }
            } else {
                ui::InteractionMode::Normal
            };

            let root_slots: Vec<_> = self.editor.doc.roots.iter().cloned().collect();

            if root_slots.is_empty() {
                if ui.button("Add root node").clicked() {
                    self.editor.selection = Some(Selection::InsertRoot(0));
                    self.insert_new_node();
                }
            } else {
                for (i, root_slot) in root_slots.iter().enumerate() {
                    let selected = matches!(self.editor.selection, Some(Selection::InsertRoot(idx)) if idx == i);
                    if ui::insertion_point(ui, selected).clicked() {
                        self.editor.selection = Some(Selection::InsertRoot(i));
                    }
                    let path = Path::new(root_slot.clone());
                    ui::project(ui, &mut self.editor, &path, &mode);
                }
                let selected = matches!(self.editor.selection, Some(Selection::InsertRoot(idx)) if idx == root_slots.len());
                if ui::insertion_point(ui, selected).clicked() {
                    self.editor.selection = Some(Selection::InsertRoot(root_slots.len()));
                }
            }

            if bg_response.clicked() {
                self.editor.selection = None;
            }
        });
    }
}
