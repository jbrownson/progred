mod document;
pub mod generated;
mod graph;
mod shortcuts;
mod ts_runtime;
mod ui;

use document::{Document, Editor, EditorWriter};
use eframe::egui;
use graph::{Gid, Id, MutGid, RootSlot};

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
            if let Ok(json) = serde_json::to_string_pretty(&self.editor.doc.to_json()) {
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
        if let Some((path, doc)) = rfd::FileDialog::new()
            .add_filter("Progred", &["progred"])
            .pick_file()
            .and_then(|path| {
                let contents = std::fs::read_to_string(&path).ok()?;
                Some((path, Document::from_json(&contents)?))
            })
        {
            self.editor = Editor { doc, file_path: Some(path), ..Editor::new() };
        }
    }

    fn create_type_system_semantics() -> (Document, document::Semantics) {
        let mut gid = MutGid::new();

        // === Semantic fields ===
        let name = Id::new_uuid();
        let isa = Id::new_uuid();

        // === Fields for type constructs ===
        let body_f = Id::new_uuid();     // type, forall: the type being named/parameterized
        let params_f = Id::new_uuid();   // forall: list of type params
        let base_f = Id::new_uuid();     // apply: the parameterized type
        let args_f = Id::new_uuid();     // apply: list of type arguments
        let variants_f = Id::new_uuid(); // sum: list of variant types
        let fields_f = Id::new_uuid();   // product: list of field definitions
        let type_f = Id::new_uuid();     // field def: the field's type

        // === List fields (for bootstrapping) ===
        let head_f = Id::new_uuid();
        let tail_f = Id::new_uuid();

        // === Type constructs ===
        let type_t = Id::new_uuid();     // names a type
        let forall_t = Id::new_uuid();   // introduces type params
        let apply_t = Id::new_uuid();    // instantiates type params
        let sum_t = Id::new_uuid();      // union / or
        let product_t = Id::new_uuid();  // record / and
        let field_t = Id::new_uuid();    // field definition (name + type)

        // === Primitives ===
        let string_t = Id::new_uuid();
        let number_t = Id::new_uuid();

        // === List (bootstrapped) ===
        let cons_t = Id::new_uuid();
        let empty_t = Id::new_uuid();

        // === Set names ===
        for (id, n) in [
            (&name, "name"), (&isa, "isa"),
            (&body_f, "body"), (&params_f, "params"), (&base_f, "base"),
            (&args_f, "args"), (&variants_f, "variants"), (&fields_f, "fields"),
            (&type_f, "type"), (&head_f, "head"), (&tail_f, "tail"),
            (&type_t, "type"), (&forall_t, "forall"), (&apply_t, "apply"),
            (&sum_t, "sum"), (&product_t, "product"), (&field_t, "field"),
            (&string_t, "string"), (&number_t, "number"),
            (&cons_t, "cons"), (&empty_t, "empty"),
        ] {
            gid.set(id.clone(), name.clone(), Id::String(n.into()));
        }

        // === Set isa: type constructs are products ===
        for id in [&type_t, &forall_t, &apply_t, &sum_t, &product_t, &field_t, &cons_t, &empty_t] {
            gid.set(id.clone(), isa.clone(), product_t.clone());
        }
        // Fields are fields
        for id in [&name, &isa, &body_f, &params_f, &base_f, &args_f, &variants_f, &fields_f, &type_f, &head_f, &tail_f] {
            gid.set(id.clone(), isa.clone(), field_t.clone());
        }
        // Primitives are types (named types with no body needed)
        for id in [&string_t, &number_t] {
            gid.set(id.clone(), isa.clone(), type_t.clone());
        }

        // === Helper to make lists ===
        let make_list = |gid: &mut MutGid, elements: &[&Id]| -> Id {
            elements.iter().rev().fold(Id::new_uuid(), |tail_node, &element| {
                if matches!(tail_node, Id::Uuid(_)) && gid.edges(&tail_node).is_none() {
                    gid.set(tail_node.clone(), isa.clone(), empty_t.clone());
                }
                let node = Id::new_uuid();
                gid.set(node.clone(), isa.clone(), cons_t.clone());
                gid.set(node.clone(), head_f.clone(), element.clone());
                gid.set(node.clone(), tail_f.clone(), tail_node);
                node
            })
        };

        // === Define fields for each construct ===
        // type has: name, body
        let type_fields = make_list(&mut gid, &[&name, &body_f]);
        gid.set(type_t.clone(), fields_f.clone(), type_fields);

        // forall has: params, body
        let forall_fields = make_list(&mut gid, &[&params_f, &body_f]);
        gid.set(forall_t.clone(), fields_f.clone(), forall_fields);

        // apply has: base, args
        let apply_fields = make_list(&mut gid, &[&base_f, &args_f]);
        gid.set(apply_t.clone(), fields_f.clone(), apply_fields);

        // sum has: variants
        let sum_fields = make_list(&mut gid, &[&variants_f]);
        gid.set(sum_t.clone(), fields_f.clone(), sum_fields);

        // product has: fields
        let product_fields = make_list(&mut gid, &[&fields_f]);
        gid.set(product_t.clone(), fields_f.clone(), product_fields);

        // field has: name, type
        let field_fields = make_list(&mut gid, &[&name, &type_f]);
        gid.set(field_t.clone(), fields_f.clone(), field_fields);

        // cons has: head, tail
        let cons_fields = make_list(&mut gid, &[&head_f, &tail_f]);
        gid.set(cons_t.clone(), fields_f.clone(), cons_fields);

        // empty has no fields
        let empty_fields = make_list(&mut gid, &[]);
        gid.set(empty_t.clone(), fields_f.clone(), empty_fields);

        let roots = vec![
            RootSlot::new(type_t),
            RootSlot::new(forall_t),
            RootSlot::new(apply_t),
            RootSlot::new(sum_t),
            RootSlot::new(product_t),
            RootSlot::new(field_t),
            RootSlot::new(string_t),
            RootSlot::new(number_t),
        ];

        let semantics = document::Semantics {
            name_field: Some(name),
            isa_field: Some(isa),
            cons_variant: Some(cons_t),
            empty_variant: Some(empty_t),
            head_field: Some(head_f),
            tail_field: Some(tail_f),
        };

        (Document { gid, roots }, semantics)
    }

    fn create_standard_semantics() -> (Document, document::Semantics) {
        let mut gid = MutGid::new();

        // === Semantic fields (editor-recognized) ===
        let name = Id::new_uuid();
        let isa = Id::new_uuid();

        // === Type system fields ===
        let fields_f = Id::new_uuid();
        let variants_f = Id::new_uuid();
        let type_f = Id::new_uuid();
        let head_f = Id::new_uuid();
        let tail_f = Id::new_uuid();

        // === Enums (type definitions) ===
        let enum_e = Id::new_uuid();
        let variant_e = Id::new_uuid();
        let field_e = Id::new_uuid();
        let list_e = Id::new_uuid();

        // === List variants ===
        let empty_v = Id::new_uuid();
        let cons_v = Id::new_uuid();

        // === Set names ===
        for (id, n) in [
            (&name, "name"),
            (&isa, "isa"),
            (&fields_f, "fields"),
            (&variants_f, "variants"),
            (&type_f, "type"),
            (&head_f, "head"),
            (&tail_f, "tail"),
            (&enum_e, "enum"),
            (&variant_e, "variant"),
            (&field_e, "field"),
            (&list_e, "list"),
            (&empty_v, "empty"),
            (&cons_v, "cons"),
        ] {
            gid.set(id.clone(), name.clone(), Id::String(n.into()));
        }

        // === Set types (isa) ===
        // All fields are fields
        for id in [&name, &isa, &fields_f, &variants_f, &type_f, &head_f, &tail_f] {
            gid.set(id.clone(), isa.clone(), field_e.clone());
        }
        // All enums are enums
        for id in [&enum_e, &variant_e, &field_e, &list_e] {
            gid.set(id.clone(), isa.clone(), enum_e.clone());
        }
        // List variants are variants
        for id in [&empty_v, &cons_v] {
            gid.set(id.clone(), isa.clone(), variant_e.clone());
        }

        // === Self-description: what fields each type has ===
        // Helper to create a list
        let make_list = |gid: &mut MutGid, elements: &[&Id]| -> Id {
            elements.iter().rev().fold(Id::new_uuid(), |tail_node, &element| {
                if matches!(tail_node, Id::Uuid(_)) && gid.edges(&tail_node).is_none() {
                    // First iteration: tail_node is fresh, make it empty
                    gid.set(tail_node.clone(), isa.clone(), empty_v.clone());
                }
                let node = Id::new_uuid();
                gid.set(node.clone(), isa.clone(), cons_v.clone());
                gid.set(node.clone(), head_f.clone(), element.clone());
                gid.set(node.clone(), tail_f.clone(), tail_node);
                node
            })
        };

        // enum has fields: [name, variants]
        let enum_fields = make_list(&mut gid, &[&name, &variants_f]);
        gid.set(enum_e.clone(), fields_f.clone(), enum_fields);

        // variant has fields: [name, fields]
        let variant_fields = make_list(&mut gid, &[&name, &fields_f]);
        gid.set(variant_e.clone(), fields_f.clone(), variant_fields);

        // field has fields: [name, type]
        let field_fields = make_list(&mut gid, &[&name, &type_f]);
        gid.set(field_e.clone(), fields_f.clone(), field_fields);

        // list has variants: [empty, cons]
        let list_variants = make_list(&mut gid, &[&empty_v, &cons_v]);
        gid.set(list_e.clone(), variants_f.clone(), list_variants);

        // cons has fields: [head, tail]
        let cons_fields = make_list(&mut gid, &[&head_f, &tail_f]);
        gid.set(cons_v.clone(), fields_f.clone(), cons_fields);

        // empty has no fields (empty list)
        let empty_fields = Id::new_uuid();
        gid.set(empty_fields.clone(), isa.clone(), empty_v.clone());
        gid.set(empty_v.clone(), fields_f.clone(), empty_fields);

        let roots = vec![
            RootSlot::new(enum_e),
            RootSlot::new(variant_e),
            RootSlot::new(field_e),
            RootSlot::new(list_e),
        ];

        let semantics = document::Semantics {
            name_field: Some(name),
            isa_field: Some(isa),
            cons_variant: Some(cons_v),
            empty_variant: Some(empty_v),
            head_field: Some(head_f),
            tail_field: Some(tail_f),
        };

        (Document { gid, roots }, semantics)
    }

    fn load_standard_semantics(&mut self) {
        let (doc, semantics) = Self::create_standard_semantics();
        self.editor = Editor { doc, semantics, ..Editor::new() };
    }

    fn load_type_system_semantics(&mut self) {
        let (doc, semantics) = Self::create_type_system_semantics();
        self.editor = Editor { doc, semantics, ..Editor::new() };
    }

    fn delete_selection(&mut self) {
        if let Some(selection) = self.editor.selection.take() {
            self.editor.doc.delete(&selection.target);
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
            .map_or(false, |s| s.placeholder_visible(&self.editor.doc));
        let editing = placeholder_active || self.editor.editing_leaf;
        ctx.input_mut(|i| {
            if i.consume_shortcut(&shortcuts::SAVE_AS) {
                self.save_as();
            } else if i.consume_shortcut(&shortcuts::SAVE) {
                self.save();
            } else if !editing {
                if i.key_pressed(egui::Key::Escape) {
                    self.editor.selection = None;
                } else if i.key_pressed(egui::Key::Delete) || i.consume_shortcut(&shortcuts::DELETE) {
                    self.delete_selection();
                } else if i.consume_shortcut(&shortcuts::INSERT_NODE) {
                    self.insert_new_node();
                } else if i.consume_shortcut(&shortcuts::NEW) {
                    self.new_document();
                } else if i.consume_shortcut(&shortcuts::OPEN) {
                    self.open();
                }
            }
        });
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context) {
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
                    ui.separator();
                    if ui.add(egui::Button::new("Load Standard Semantics")).clicked() {
                        self.load_standard_semantics();
                        ui.close();
                    }
                    if ui.add(egui::Button::new("Load Type System")).clicked() {
                        self.load_type_system_semantics();
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.edge_path()).is_some(),
                        egui::Button::new("New Node").shortcut_text(shortcuts::format(&shortcuts::INSERT_NODE)),
                    ).clicked() {
                        self.insert_new_node();
                        ui.close();
                    }
                    if ui.add_enabled(
                        self.editor.selection.as_ref().and_then(|s| s.edge_path()).is_some(),
                        egui::Button::new("Delete").shortcut_text(shortcuts::format(&shortcuts::DELETE)),
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
                self.editor.refresh_orphan_cache();
                let snapshot = self.editor.clone();
                let mut w = EditorWriter::new(&mut self.editor);

                if self.show_graph {
                    let rects = ui::split_view::horizontal_split(ui, ctx, &mut self.graph_split);
                    ui::split_view::scoped(ui, rects.left, |ui| ui::tree_view::render(ui, ctx, &snapshot, &mut w));
                    ui::split_view::scoped(ui, rects.right, |ui| ui::graph_view::render(ui, ctx, &snapshot, &mut w));
                } else {
                    ui::split_view::scoped(ui, ui.max_rect(), |ui| ui::tree_view::render(ui, ctx, &snapshot, &mut w));
                }
            });
    }
}

