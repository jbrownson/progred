mod id;
mod mutgid;
mod path;
mod spanningtree;

use eframe::egui;
use id::{GuidId, Id, StringId};
use mutgid::MutGid;
use spanningtree::SpanningTree;

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

struct RootSlot {
    id: GuidId,
    node: Option<Id>,
}

struct ProgredApp {
    gid: MutGid,
    roots: Vec<RootSlot>,
    tree: SpanningTree,
    name_label: Option<GuidId>,
    isa_label: Option<GuidId>,
}

impl ProgredApp {
    fn new() -> Self {
        let (gid, roots, name_label, isa_label) = Self::create_test_data();
        Self {
            gid,
            roots,
            tree: SpanningTree::empty(),
            name_label: Some(name_label),
            isa_label: Some(isa_label),
        }
    }

    fn create_test_data() -> (MutGid, Vec<RootSlot>, GuidId, GuidId) {
        let mut gid = MutGid::new();

        // Bootstrap: define 'field' and 'name'/'isa' as fields
        let field = GuidId::generate();
        let name = GuidId::generate();
        let isa = GuidId::generate();

        // field is-a field, named "field"
        gid.set(
            field.clone(),
            isa.clone(),
            Id::Guid(field.clone()),
        );
        gid.set(
            field.clone(),
            name.clone(),
            Id::String(StringId::new("field".to_string())),
        );

        // name is-a field, named "name"
        gid.set(
            name.clone(),
            isa.clone(),
            Id::Guid(field.clone()),
        );
        gid.set(
            name.clone(),
            name.clone(),
            Id::String(StringId::new("name".to_string())),
        );

        // isa is-a field, named "isa"
        gid.set(
            isa.clone(),
            isa.clone(),
            Id::Guid(field.clone()),
        );
        gid.set(
            isa.clone(),
            name.clone(),
            Id::String(StringId::new("isa".to_string())),
        );

        let roots = vec![
            RootSlot {
                id: GuidId::generate(),
                node: Some(Id::Guid(field)),
            },
            RootSlot {
                id: GuidId::generate(),
                node: Some(Id::Guid(name.clone())),
            },
            RootSlot {
                id: GuidId::generate(),
                node: Some(Id::Guid(isa.clone())),
            },
        ];

        (gid, roots, name, isa)
    }
}

impl eframe::App for ProgredApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Progred - Graph Editor");

            ui.separator();

            // Display roots
            for root_slot in &self.roots {
                if let Some(node) = &root_slot.node {
                    ui.label(format!("Root: {}", node));
                }
            }

            ui.separator();
            ui.label(format!("Entities in graph: {}", self.gid.entities().count()));
        });
    }
}
