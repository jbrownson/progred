mod graph;
mod ts_runtime;
mod ui;

use eframe::egui;
use graph::{Id, MutGid, Path, RootSlot, SpanningTree};

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
}

impl ProgredApp {
    fn new() -> Self {
        let (gid, roots) = Self::create_test_data();
        Self {
            gid,
            roots,
            tree: SpanningTree::empty(),
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

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Progred - Graph Editor");

            ui.separator();

            let root_slots: Vec<_> = self.roots.iter().cloned().collect();
            for root_slot in root_slots {
                let path = Path::new(root_slot);
                ui::project(ui, &self.gid, &mut self.tree, &path);
                ui.add_space(8.0);
            }

            ui.separator();
            ui.label(format!("Entities in graph: {}", self.gid.entities().count()));
        });
    }
}
