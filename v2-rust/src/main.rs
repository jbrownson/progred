mod id;
mod identicon;
mod mutgid;
mod path;
mod spanningtree;
mod ts_runtime;

use eframe::egui;
use id::Id;
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
    id: Id,
    node: Option<Id>,
}

struct ProgredApp {
    gid: MutGid,
    roots: Vec<RootSlot>,
    tree: SpanningTree,
    name_label: Option<Id>,
    isa_label: Option<Id>,
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

    fn create_test_data() -> (MutGid, Vec<RootSlot>, Id, Id) {
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
            RootSlot {
                id: Id::new_uuid(),
                node: Some(field),
            },
            RootSlot {
                id: Id::new_uuid(),
                node: Some(name.clone()),
            },
            RootSlot {
                id: Id::new_uuid(),
                node: Some(isa.clone()),
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

            for root_slot in &self.roots {
                if let Some(node) = &root_slot.node {
                    ui.horizontal(|ui| {
                        let size = 20.0;
                        let (rect, _response) =
                            ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                        if let Id::Uuid(uuid) = node {
                            identicon::paint_identicon(ui.painter(), rect, uuid);
                        }

                        let label = match node {
                            Id::Uuid(_) => self
                                .name_label
                                .as_ref()
                                .and_then(|name_label| self.gid.get(node, name_label))
                                .and_then(|id| match id {
                                    Id::String(s) => Some(s.clone()),
                                    _ => None,
                                })
                                .unwrap_or_else(|| format!("{}", node)),
                            _ => format!("{}", node),
                        };
                        ui.label(label);
                    });
                }
            }

            ui.separator();
            ui.label(format!("Entities in graph: {}", self.gid.entities().count()));
        });
    }
}
