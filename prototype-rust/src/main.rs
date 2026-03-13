mod shortcuts;
mod ui;

use progred_core::document::Document;
use progred_core::editor::Editor;
use progred_core::graph_view_state::GraphViewState;
use progred_core::navigate::{self, DescendNode};
use eframe::egui;
use progred_core::graph::Id;
use progred_core::selection::Selection;

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
    graph_layout: GraphViewState,
    graph_camera: ui::graph_view::CameraState,
}

impl ProgredApp {
    fn new() -> Self {
        Self {
            editor: Editor::new(),
            show_graph: false,
            graph_split: 0.5,
            graph_layout: GraphViewState::new(),
            graph_camera: ui::graph_view::CameraState::new(),
        }
    }

    fn new_document(&mut self) {
        self.editor = Editor::new();
        self.graph_layout = GraphViewState::new();
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
        if let Some(ref path) = self.editor.file_path
            && let Ok(json) = serde_json::to_string_pretty(&self.editor.doc)
        {
            let _ = std::fs::write(path, json);
        }
    }

    fn window_title(&self) -> String {
        self.editor.file_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map_or_else(|| "Progred".to_string(), |name| format!("{} - Progred", name))
    }

    fn open(&mut self) {
        if let Some((path, doc)) = rfd::FileDialog::new()
            .add_filter("Progred", &["progred"])
            .pick_file()
            .and_then(|path| {
                let contents = std::fs::read_to_string(&path).ok()?;
                let doc: Document = serde_json::from_str(&contents).ok()?;
                Some((path, doc))
            })
        {
            self.editor = Editor { doc, file_path: Some(path), ..Editor::new() };
            self.graph_layout = GraphViewState::new();
        }
    }

    fn delete_selection(&mut self, nav: &[DescendNode]) {
        if let Some(selection) = self.editor.selection.clone() {
            let next = selection.path().and_then(|p| navigate::post_delete(nav, p));
            self.editor.doc.delete(&selection);
            self.editor.selection = next;
        }
    }

    fn insert_new_node(&mut self) {
        if let Some(path) = self.editor.selection.as_ref().and_then(|s| s.path()).cloned() {
            self.editor.doc.set_edge(&path, Id::new_uuid());
            self.editor.selection = None;
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context, nav: &[DescendNode]) {
        ctx.input_mut(|i| {
            if i.consume_shortcut(&shortcuts::SAVE_AS) {
                self.save_as();
            } else if i.consume_shortcut(&shortcuts::SAVE) {
                self.save();
            }
        });
        if !self.placeholder_handler(ctx)
            && !self.leaf_edit_handler()
            && !self.nav_handler(ctx, nav)
        {
            self.global_handler(ctx, nav);
        }
    }

    fn placeholder_handler(&mut self, ctx: &egui::Context) -> bool {
        let active = match &self.editor.selection {
            Some(Selection::Edge(path, _)) => self.editor.doc.node(path).is_none(),
            _ => false,
        };
        if !active { return false; }
        ctx.input_mut(|i| {
            if i.key_pressed(egui::Key::Escape) {
                self.editor.selection = None;
            }
        });
        true
    }

    fn leaf_edit_handler(&self) -> bool {
        match &self.editor.selection {
            Some(Selection::Edge(path, _)) => {
                matches!(self.editor.doc.node(path), Some(Id::String(_) | Id::Number(_)))
            }
            _ => false,
        }
    }

    fn nav_handler(&mut self, ctx: &egui::Context, nav: &[DescendNode]) -> bool {
        let current = match self.editor.selection.as_ref().and_then(|s| s.path()) {
            Some(p) => p.clone(),
            None => {
                return ctx.input_mut(|i| {
                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        self.editor.selection = navigate::first_placeholder(nav)
                            .or_else(|| nav.first().map(|n| n.selection.clone()));
                        true
                    } else {
                        false
                    }
                });
            }
        };
        ctx.input_mut(|i| {
            let new_sel = if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                navigate::arrow_down(nav, &current)
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                navigate::arrow_up(nav, &current)
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                navigate::arrow_left(nav, &current)
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                navigate::arrow_right(nav, &current)
            } else {
                return false;
            };
            if let Some(sel) = new_sel {
                self.editor.selection = Some(sel);
            }
            true
        })
    }

    fn global_handler(&mut self, ctx: &egui::Context, nav: &[DescendNode]) {
        ctx.input_mut(|i| {
            if i.key_pressed(egui::Key::Escape) {
                self.editor.selection = None;
            } else if i.key_pressed(egui::Key::Delete) || i.consume_shortcut(&shortcuts::DELETE) {
                self.delete_selection(nav);
            } else if i.consume_shortcut(&shortcuts::INSERT_NODE) {
                self.insert_new_node();
            } else if i.consume_shortcut(&shortcuts::NEW) {
                self.new_document();
            } else if i.consume_shortcut(&shortcuts::OPEN) {
                self.open();
            }
        });
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context, nav: &[DescendNode]) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New").shortcut_text(shortcuts::format(&shortcuts::NEW))).clicked() {
                        self.new_document();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Open...").shortcut_text(shortcuts::format(&shortcuts::OPEN))).clicked() {
                        self.open();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Save").shortcut_text(shortcuts::format(&shortcuts::SAVE))).clicked() {
                        self.save();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Save As...").shortcut_text(shortcuts::format(&shortcuts::SAVE_AS))).clicked() {
                        self.save_as();
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.path()).is_some(),
                        egui::Button::new("New Node").shortcut_text(shortcuts::format(&shortcuts::INSERT_NODE)),
                    ).clicked() {
                        self.insert_new_node();
                        ui.close();
                    }
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.path()).is_some(),
                        egui::Button::new("Delete").shortcut_text(shortcuts::format(&shortcuts::DELETE)),
                    ).clicked() {
                        self.delete_selection(nav);
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

        let d_tree = self.editor.render_d_tree();
        let nav = navigate::collect_descends(&d_tree);
        let orphan_ids = self.editor.doc.orphan_roots();
        self.handle_keys(ctx, &nav);

        self.render_menu_bar(ctx, &nav);

        if self.show_graph {
            progred_core::graph_view_state::step_physics(&mut self.graph_layout, &self.editor.doc, self.graph_camera.dragging());
        }

        let modifiers = ctx.input(|i| i.modifiers);
        let mode = ui::compute_interaction_mode(modifiers, &self.editor);
        let mut events = Vec::new();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                if self.show_graph {
                    ui::split_view::horizontal_split(ui, ctx, &mut self.graph_split, |left, right| {
                        ui::tree_view::render(left, &self.editor, &d_tree, &orphan_ids, &mode, &mut events);
                        ui::graph_view::render(right, ctx, &self.editor, &mut self.graph_layout, &mut self.graph_camera, &mut events);
                    });
                } else {
                    ui::tree_view::render(ui, &self.editor, &d_tree, &orphan_ids, &mode, &mut events);
                }
            });

        self.editor.handle_events(events, &mode);
    }
}
