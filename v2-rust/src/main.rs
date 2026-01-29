mod document;
mod graph;
mod ts_runtime;
mod ui;

use document::{Document, Editor, EditorWriter};
use eframe::egui;
use graph::{Id, MutGid, RootSlot};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Progred"),
        vsync: false,
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
    show_graph: bool,
    graph_split: f32,
}

impl ProgredApp {
    fn new() -> Self {
        Self {
            editor: Editor::new(),
            show_graph: false,
            graph_split: 0.5,
        }
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
        if let Some(path) = self.editor.selection.as_ref().and_then(|s| s.edge_path()) {
            self.editor.doc.set_edge(path, Id::new_uuid());
            self.editor.selection = None;
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let placeholder_active = self.editor.selection.as_ref()
            .map_or(false, |s| s.placeholder_visible(&self.editor.doc.gid));
        ctx.input(|i| {
            if placeholder_active {
                if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S) {
                    self.save_as();
                } else if i.modifiers.command && i.key_pressed(egui::Key::S) {
                    self.save();
                }
            } else if i.key_pressed(egui::Key::Escape) {
                self.editor.selection = None;
            } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                self.delete_selection();
            } else if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::N) {
                self.insert_new_node();
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
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New").shortcut_text("Cmd+N")).clicked() {
                        self.new_document();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Open...").shortcut_text("Cmd+O")).clicked() {
                        self.open();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Save").shortcut_text("Cmd+S")).clicked() {
                        self.save();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Save As...").shortcut_text("Shift+Cmd+S")).clicked() {
                        self.save_as();
                        ui.close();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Load Test Data")).clicked() {
                        self.load_test_data();
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.edge_path()).is_some(),
                        egui::Button::new("New Node").shortcut_text("Shift+Cmd+N"),
                    ).clicked() {
                        self.insert_new_node();
                        ui.close();
                    }
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.edge_path()).is_some(),
                        egui::Button::new("Delete").shortcut_text("Backspace"),
                    ).clicked() {
                        self.delete_selection();
                        ui.close();
                    }
                });
                ui.separator();
                ui.toggle_value(&mut self.show_graph, "Graph");
            });
        });
    }
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));
        self.handle_shortcuts(ctx);
        self.render_menu_bar(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                let snapshot = self.editor.clone();
                let mut w = EditorWriter::new(&mut self.editor);

                if self.show_graph {
                    let rects = ui::split_view::horizontal_split(ui, ctx, &mut self.graph_split);
                    ui::split_view::scoped_with_margin(ui, rects.left, 4.0, |ui| ui::tree_view::render(ui, ctx, &snapshot, &mut w));
                    ui::split_view::scoped(ui, rects.right, |ui| ui::graph_view::render(ui, ctx, &snapshot, &mut w));
                } else {
                    ui::split_view::scoped_with_margin(ui, ui.max_rect(), 4.0, |ui| ui::tree_view::render(ui, ctx, &snapshot, &mut w));
                }
            });
    }
}

