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
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            self.delete_selection();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let bg_response = ui.interact(
                ui.max_rect(),
                ui.id().with("background"),
                egui::Sense::click(),
            );

            ui.heading("Progred - Graph Editor");

            ui.separator();

            let root_slots: Vec<_> = self.roots.iter().cloned().collect();
            for root_slot in root_slots {
                let path = Path::new(root_slot);
                ui::project(ui, &self.gid, &mut self.tree, &mut self.selection, &path);
                ui.add_space(8.0);
            }

            if let Some(ref sel) = self.selection {
                ui.separator();
                ui.label(format!("Selection: {:?}", sel));
            }

            ui.separator();
            ui.label(format!("Entities in graph: {}", self.gid.entities().count()));

            if bg_response.clicked() {
                self.selection = None;
            }
        });
    }
}
