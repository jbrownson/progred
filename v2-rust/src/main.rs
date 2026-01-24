mod graph;
mod ts_runtime;
mod ui;

use eframe::egui;
use graph::{Id, MutGid, Path, RootSlot, Selection, SpanningTree};
use std::path::PathBuf;

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
    gid: MutGid,
    roots: Vec<RootSlot>,
    tree: SpanningTree,
    selection: Option<Selection>,
    file_path: Option<PathBuf>,
}

impl ProgredApp {
    fn new() -> Self {
        Self {
            gid: MutGid::new(),
            roots: Vec::new(),
            tree: SpanningTree::empty(),
            selection: None,
            file_path: None,
        }
    }

    fn new_document(&mut self) {
        self.gid = MutGid::new();
        self.roots = Vec::new();
        self.tree = SpanningTree::empty();
        self.selection = None;
        self.file_path = None;
    }

    fn save(&mut self) {
        if self.file_path.is_some() {
            self.save_to_path();
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Progred", &["progred"]).save_file() {
            self.file_path = Some(path);
            self.save_to_path();
        }
    }

    fn save_to_path(&self) {
        if let Some(ref path) = self.file_path {
            let root_ids: Vec<_> = self.roots.iter().map(|r| r.node().clone()).collect();
            let doc = serde_json::json!({
                "graph": self.gid.to_json(),
                "roots": root_ids,
            });
            if let Ok(json) = serde_json::to_string_pretty(&doc) {
                // TODO: show error to user if write fails
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn window_title(&self) -> String {
        match &self.file_path {
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
                let doc: serde_json::Value = serde_json::from_str(&contents).ok()?;
                let graph_data = serde_json::from_value(doc.get("graph")?.clone()).ok()?;
                let gid = MutGid::from_json(graph_data).ok()?;
                let root_ids: Vec<Id> = serde_json::from_value(doc.get("roots")?.clone()).ok()?;
                Some((path, gid, root_ids))
            });

        if let Some((path, gid, root_ids)) = result {
            self.gid = gid;
            self.roots = root_ids.into_iter().map(RootSlot::new).collect();
            self.tree = SpanningTree::empty();
            self.selection = None;
            self.file_path = Some(path);
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
        self.gid = gid;
        self.roots = roots;
        self.tree = SpanningTree::empty();
        self.selection = None;
        self.file_path = None;
    }

    fn delete_path(&mut self, path: &Path) {
        match path.pop() {
            None => {
                if let Some(idx) = self.roots.iter().position(|r| r == &path.root) {
                    self.roots.remove(idx);
                }
            }
            Some((parent_path, label)) => {
                if let Some(parent_node) = parent_path.node(&self.gid).cloned() {
                    if let Id::Uuid(_) = parent_node {
                        self.gid.delete(&parent_node, &label);
                    }
                }
            }
        }
    }

    fn delete_selection(&mut self) {
        if let Some(Selection::Edge(ref path)) = self.selection.clone() {
            self.delete_path(&path);
            self.selection = None;
        }
    }

    fn insert_new_node(&mut self) {
        match &self.selection {
            Some(Selection::InsertRoot(index)) => {
                let new_id = Id::new_uuid();
                let index = (*index).min(self.roots.len());
                self.roots.insert(index, RootSlot::new(new_id));
                self.selection = None;
            }
            Some(Selection::Edge(path)) => {
                let new_id = Id::new_uuid();
                match path.pop() {
                    Some((parent_path, label)) => {
                        if let Some(parent_node) = parent_path.node(&self.gid).cloned() {
                            if let Id::Uuid(_) = parent_node {
                                self.gid.set(parent_node, label, new_id);
                            }
                        }
                    }
                    None => {
                        if let Some(idx) = self.roots.iter().position(|r| r == &path.root) {
                            self.roots[idx] = RootSlot::new(new_id);
                        }
                    }
                }
                self.selection = None;
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
                self.selection = None;
            } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                self.delete_selection();
            } else if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::N) {
                if self.selection.is_some() { self.insert_new_node(); }
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
                    let can_insert = self.selection.is_some();
                    let can_delete = matches!(self.selection, Some(Selection::Edge(_)));
                    
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

            let root_slots: Vec<_> = self.roots.iter().cloned().collect();
            
            if root_slots.is_empty() {
                if ui.button("Add root node").clicked() {
                    self.selection = Some(Selection::InsertRoot(0));
                    self.insert_new_node();
                }
            } else {
                for (i, root_slot) in root_slots.iter().enumerate() {
                    ui::insertion_point(ui, &mut self.selection, i);
                    let path = Path::new(root_slot.clone());
                    ui::project(ui, &self.gid, &mut self.tree, &mut self.selection, &path);
                }
                ui::insertion_point(ui, &mut self.selection, root_slots.len());
            }

            if bg_response.clicked() {
                self.selection = None;
            }
        });
    }
}
