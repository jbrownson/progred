mod graph;
mod ts_runtime;
mod ui;

use eframe::egui;
use graph::{Id, MutGid, Path, RootSlot, Selection, SpanningTree};

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
}

impl ProgredApp {
    fn new() -> Self {
        let (gid, roots) = Self::create_test_data();
        Self {
            gid,
            roots,
            tree: SpanningTree::empty(),
            selection: None,
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
}

impl ProgredApp {
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
                if let Some((parent_path, label)) = path.pop() {
                    if let Some(parent_node) = parent_path.node(&self.gid).cloned() {
                        if let Id::Uuid(_) = parent_node {
                            self.gid.set(parent_node, label, new_id);
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
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            self.delete_selection();
        }

        let ctrl_shift_n = ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::N));
        if ctrl_shift_n && self.selection.is_some() {
            self.insert_new_node();
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
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
            for (i, root_slot) in root_slots.iter().enumerate() {
                ui::insertion_point(ui, &mut self.selection, i);
                let path = Path::new(root_slot.clone());
                ui::project(ui, &self.gid, &mut self.tree, &mut self.selection, &path);
            }
            ui::insertion_point(ui, &mut self.selection, root_slots.len());

            if bg_response.clicked() {
                self.selection = None;
            }
        });
    }
}
