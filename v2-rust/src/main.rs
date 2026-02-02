mod document;
pub mod generated;
mod graph;
mod shortcuts;
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
        if let Some(ref path) = self.editor.file_path
            && let Ok(json) = serde_json::to_string_pretty(&self.editor.doc.to_json())
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
                Some((path, Document::from_json(&contents)?))
            })
        {
            let semantics = document::Semantics::detect(&doc);
            self.editor = Editor { doc, file_path: Some(path), semantics, ..Editor::new() };
        }
    }

    fn create_type_system_semantics() -> (Document, document::Semantics) {
        use crate::generated::semantics::*;

        fn id(s: &str) -> Id {
            Id::Uuid(uuid::Uuid::parse_str(s).unwrap())
        }

        let mut gid = MutGid::new();

        // === Semantic fields (from saved schema) ===
        let name = id(Field::NAME);
        let isa = id(Field::ISA);

        // === Fields for type constructs ===
        let body_f = id(Field::BODY);
        let params_f = id(Field::PARAMS);
        let base_f = id(Field::BASE);
        let args_f = id(Field::ARGS);
        let variants_f = id(Field::VARIANTS);
        let fields_f = id(Field::FIELDS);
        let type_f = id(Field::TYPE_);
        let head_f = id(Field::HEAD);
        let tail_f = id(Field::TAIL);

        // === Type constructs (from saved schema) ===
        let type_t = id(Type::TYPE_ID);
        let forall_t = id(Forall::TYPE_ID);
        let apply_t = id(Apply::TYPE_ID);
        let sum_t = id(Sum::TYPE_ID);
        let record_t = id(Record::TYPE_ID);
        let field_t = id(Field::TYPE_ID);

        // === Primitives ===
        let string_t = id(STRING_TYPE);
        let number_t = id(NUMBER_TYPE);

        // === List (bootstrapped) ===
        let cons_t = id(CONS_TYPE);
        let empty_t = id(EMPTY_TYPE);
        let list_t = id(LIST_TYPE);
        let list_param_t = Id::new_uuid(); // The T type parameter (internal to list definition)

        // === Helper to make lists ===
        // Note: uses cons_t/empty_t before fully defined, just needs IDs
        let make_list = |gid: &mut MutGid, elements: &[&Id]| -> Id {
            let empty_node = Id::new_uuid();
            gid.set(empty_node.clone(), isa.clone(), empty_t.clone());
            elements.iter().rev().fold(empty_node, |tail_node, &element| {
                let node = Id::new_uuid();
                gid.set(node.clone(), isa.clone(), cons_t.clone());
                gid.set(node.clone(), head_f.clone(), element.clone());
                gid.set(node.clone(), tail_f.clone(), tail_node);
                node
            })
        };

        // === All type constructs are: type "name" { body: record/sum/forall/apply { ... } } ===
        // The stable IDs (type_t, forall_t, etc.) are the type wrappers.
        // Each has isa=type_t (type_t is self-referential) and a body.

        // --- type "type" { body: record { fields: [name, body] } } ---
        gid.set(type_t.clone(), isa.clone(), type_t.clone()); // self-referential
        gid.set(type_t.clone(), name.clone(), Id::String("type".into()));
        let type_body = Id::new_uuid();
        gid.set(type_body.clone(), isa.clone(), record_t.clone());
        let type_fields = make_list(&mut gid, &[&name, &body_f]);
        gid.set(type_body.clone(), fields_f.clone(), type_fields);
        gid.set(type_t.clone(), body_f.clone(), type_body);

        // --- type "record" { body: record { fields: [fields] } } ---
        gid.set(record_t.clone(), isa.clone(), type_t.clone());
        gid.set(record_t.clone(), name.clone(), Id::String("record".into()));
        let record_body = Id::new_uuid();
        gid.set(record_body.clone(), isa.clone(), record_t.clone());
        let record_fields = make_list(&mut gid, &[&fields_f]);
        gid.set(record_body.clone(), fields_f.clone(), record_fields);
        gid.set(record_t.clone(), body_f.clone(), record_body);

        // --- type "sum" { body: record { fields: [variants] } } ---
        gid.set(sum_t.clone(), isa.clone(), type_t.clone());
        gid.set(sum_t.clone(), name.clone(), Id::String("sum".into()));
        let sum_body = Id::new_uuid();
        gid.set(sum_body.clone(), isa.clone(), record_t.clone());
        let sum_fields = make_list(&mut gid, &[&variants_f]);
        gid.set(sum_body.clone(), fields_f.clone(), sum_fields);
        gid.set(sum_t.clone(), body_f.clone(), sum_body);

        // --- type "forall" { body: record { fields: [params, body] } } ---
        gid.set(forall_t.clone(), isa.clone(), type_t.clone());
        gid.set(forall_t.clone(), name.clone(), Id::String("forall".into()));
        let forall_body = Id::new_uuid();
        gid.set(forall_body.clone(), isa.clone(), record_t.clone());
        let forall_fields = make_list(&mut gid, &[&params_f, &body_f]);
        gid.set(forall_body.clone(), fields_f.clone(), forall_fields);
        gid.set(forall_t.clone(), body_f.clone(), forall_body);

        // --- type "apply" { body: record { fields: [base, args] } } ---
        gid.set(apply_t.clone(), isa.clone(), type_t.clone());
        gid.set(apply_t.clone(), name.clone(), Id::String("apply".into()));
        let apply_body = Id::new_uuid();
        gid.set(apply_body.clone(), isa.clone(), record_t.clone());
        let apply_fields = make_list(&mut gid, &[&base_f, &args_f]);
        gid.set(apply_body.clone(), fields_f.clone(), apply_fields);
        gid.set(apply_t.clone(), body_f.clone(), apply_body);

        // --- type "field" { body: record { fields: [name, type] } } ---
        gid.set(field_t.clone(), isa.clone(), type_t.clone());
        gid.set(field_t.clone(), name.clone(), Id::String("field".into()));
        let field_body = Id::new_uuid();
        gid.set(field_body.clone(), isa.clone(), record_t.clone());
        let field_fields = make_list(&mut gid, &[&name, &type_f]);
        gid.set(field_body.clone(), fields_f.clone(), field_fields);
        gid.set(field_t.clone(), body_f.clone(), field_body);

        // --- type "cons" { body: record { fields: [head, tail] } } ---
        gid.set(cons_t.clone(), isa.clone(), type_t.clone());
        gid.set(cons_t.clone(), name.clone(), Id::String("cons".into()));
        let cons_body = Id::new_uuid();
        gid.set(cons_body.clone(), isa.clone(), record_t.clone());
        let cons_fields = make_list(&mut gid, &[&head_f, &tail_f]);
        gid.set(cons_body.clone(), fields_f.clone(), cons_fields);
        gid.set(cons_t.clone(), body_f.clone(), cons_body);

        // --- type "empty" { body: record { fields: [] } } ---
        gid.set(empty_t.clone(), isa.clone(), type_t.clone());
        gid.set(empty_t.clone(), name.clone(), Id::String("empty".into()));
        let empty_body = Id::new_uuid();
        gid.set(empty_body.clone(), isa.clone(), record_t.clone());
        let empty_fields = make_list(&mut gid, &[]);
        gid.set(empty_body.clone(), fields_f.clone(), empty_fields);
        gid.set(empty_t.clone(), body_f.clone(), empty_body);

        // --- type "string" { body: record { fields: [] } } ---
        gid.set(string_t.clone(), isa.clone(), type_t.clone());
        gid.set(string_t.clone(), name.clone(), Id::String("string".into()));
        let string_body = Id::new_uuid();
        gid.set(string_body.clone(), isa.clone(), record_t.clone());
        let string_fields = make_list(&mut gid, &[]);
        gid.set(string_body.clone(), fields_f.clone(), string_fields);
        gid.set(string_t.clone(), body_f.clone(), string_body);

        // --- type "number" { body: record { fields: [] } } ---
        gid.set(number_t.clone(), isa.clone(), type_t.clone());
        gid.set(number_t.clone(), name.clone(), Id::String("number".into()));
        let number_body = Id::new_uuid();
        gid.set(number_body.clone(), isa.clone(), record_t.clone());
        let number_fields = make_list(&mut gid, &[]);
        gid.set(number_body.clone(), fields_f.clone(), number_fields);
        gid.set(number_t.clone(), body_f.clone(), number_body);

        // --- type "list" { body: forall { params: [T], body: sum { variants: [empty, cons] } } } ---
        gid.set(list_t.clone(), isa.clone(), type_t.clone());
        gid.set(list_t.clone(), name.clone(), Id::String("list".into()));
        let list_forall = Id::new_uuid();
        gid.set(list_forall.clone(), isa.clone(), forall_t.clone());
        let list_params = make_list(&mut gid, &[&list_param_t]);
        gid.set(list_forall.clone(), params_f.clone(), list_params);
        let list_sum = Id::new_uuid();
        gid.set(list_sum.clone(), isa.clone(), sum_t.clone());
        let list_variants = make_list(&mut gid, &[&empty_t, &cons_t]);
        gid.set(list_sum.clone(), variants_f.clone(), list_variants);
        gid.set(list_forall.clone(), body_f.clone(), list_sum);
        gid.set(list_t.clone(), body_f.clone(), list_forall);

        // === Fields: type "name" { body: field structure } ===
        // Fields are instances of field_t (which is a type), so isa=field_t
        for (f, n) in [
            (&name, "name"), (&isa, "isa"), (&body_f, "body"), (&params_f, "params"),
            (&base_f, "base"), (&args_f, "args"), (&variants_f, "variants"),
            (&fields_f, "fields"), (&type_f, "type"), (&head_f, "head"), (&tail_f, "tail"),
        ] {
            gid.set(f.clone(), isa.clone(), field_t.clone());
            gid.set(f.clone(), name.clone(), Id::String(n.into()));
        }

        // T type parameter for list
        gid.set(list_param_t.clone(), name.clone(), Id::String("T".into()));

        // --- type "type expression" { body: sum { variants: [...] } } ---
        let type_expr = id(TYPE_EXPR);
        gid.set(type_expr.clone(), isa.clone(), type_t.clone());
        gid.set(type_expr.clone(), name.clone(), Id::String("type expression".into()));
        let te_sum = Id::new_uuid();
        gid.set(te_sum.clone(), isa.clone(), sum_t.clone());
        let te_variants = make_list(&mut gid, &[&type_t, &forall_t, &apply_t, &sum_t, &record_t, &string_t, &number_t]);
        gid.set(te_sum.clone(), variants_f.clone(), te_variants);
        gid.set(type_expr.clone(), body_f.clone(), te_sum);

        // === Helper to create inline apply(list, [arg]) ===
        let make_list_of = |gid: &mut MutGid, arg: &Id| -> Id {
            let apply_node = Id::new_uuid();
            gid.set(apply_node.clone(), isa.clone(), apply_t.clone());
            gid.set(apply_node.clone(), base_f.clone(), list_t.clone());
            let args = make_list(gid, &[arg]);
            gid.set(apply_node.clone(), args_f.clone(), args);
            apply_node
        };

        // === Set field types ===
        gid.set(name.clone(), type_f.clone(), string_t.clone());
        gid.set(isa.clone(), type_f.clone(), type_t.clone());
        gid.set(body_f.clone(), type_f.clone(), type_expr.clone());
        let params_type = make_list_of(&mut gid, &type_expr);
        gid.set(params_f.clone(), type_f.clone(), params_type);
        gid.set(base_f.clone(), type_f.clone(), type_t.clone());
        let args_type = make_list_of(&mut gid, &type_expr);
        gid.set(args_f.clone(), type_f.clone(), args_type);
        let fields_type = make_list_of(&mut gid, &field_t);
        gid.set(fields_f.clone(), type_f.clone(), fields_type);
        let variants_type = make_list_of(&mut gid, &record_t);
        gid.set(variants_f.clone(), type_f.clone(), variants_type);
        gid.set(type_f.clone(), type_f.clone(), type_expr.clone());
        gid.set(head_f.clone(), type_f.clone(), type_expr.clone());
        gid.set(tail_f.clone(), type_f.clone(), type_expr.clone()); // generic (list<T>)

        let roots = vec![
            // Primitives
            RootSlot::new(string_t),
            RootSlot::new(number_t),
            // Structure
            RootSlot::new(record_t),
            RootSlot::new(field_t),
            RootSlot::new(sum_t),
            // Abstraction
            RootSlot::new(type_t),
            RootSlot::new(forall_t),
            RootSlot::new(apply_t),
            // Utilities
            RootSlot::new(list_t),
            RootSlot::new(type_expr),
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
            .is_some_and(|s| s.placeholder_visible(&self.editor.doc));
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

