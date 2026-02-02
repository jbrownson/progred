
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// Core semantic field IDs (from saved semantics.progred)
const NAME_FIELD: &str = "21ab3439-ef04-4783-9022-3d0400de4bbb";
const ISA_FIELD: &str = "1d75e23d-7147-466a-a33d-384286fa518d";
const BODY_FIELD: &str = "26956a6c-e425-4b1d-bcd7-fc2dba14b277";
const PARAMS_FIELD: &str = "10650b24-4876-4726-bd57-b06ce8fcf874";
const BASE_FIELD: &str = "4c81a576-7858-46ed-8246-12158e0c4a7a";
const ARGS_FIELD: &str = "1d43868e-5873-45dc-a3b1-5f8cef0b410e";
const VARIANTS_FIELD: &str = "de15c5ce-f65c-4e32-a15e-94f37dbb7541";
const FIELDS_FIELD: &str = "f355ead0-6bfb-450e-a729-1735c3950a0e";
const TYPE_FIELD: &str = "7f328ac9-de07-45a5-86b3-021b2711747a";
const HEAD_FIELD: &str = "83f724d9-1008-43e3-807c-f66adc7d774f";
const TAIL_FIELD: &str = "cffc46b8-2388-4197-ad87-7e7ef446accb";

// Type construct IDs
const TYPE_T: &str = "b7185815-d553-4fbf-9025-d88643a7ba6a";
const APPLY_T: &str = "d6ad0273-886d-4e83-9ed7-e4fe5fe2f4e8";
const RECORD_T: &str = "4ae8a1de-f7f5-4733-b6ca-0e01862635e6";

// Primitive type IDs
const STRING_T: &str = "355e01b2-4cbd-413a-a89d-e05fba7c577d";
const NUMBER_T: &str = "d4325076-d33e-4fc8-b3f6-5ba85385a352";

// List bootstrap IDs
const CONS_T: &str = "264e80b0-a8e1-4fc2-a3bd-6c10ff177261";
const EMPTY_T: &str = "1dff7f8a-1d83-41fd-af2d-087b753ed008";

// Auxiliary type IDs
const LIST_T: &str = "c80d8e16-dbf4-4a83-80fd-38b2de25688f";
const TYPE_EXPR_T: &str = "8cafe8ce-d202-4d43-9c96-b2d30f4e5bb1";

type Graph = HashMap<String, HashMap<String, Value>>;

fn get_uuid(value: &Value) -> Option<&str> {
    value.get("uuid")?.as_str()
}

fn get_string(value: &Value) -> Option<&str> {
    value.get("string")?.as_str()
}

fn get_edge<'a>(graph: &'a Graph, entity: &str, label: &str) -> Option<&'a Value> {
    graph.get(entity)?.get(label)
}

fn get_name(graph: &Graph, id: &str) -> Option<String> {
    get_edge(graph, id, NAME_FIELD).and_then(get_string).map(String::from)
}

fn get_isa<'a>(graph: &'a Graph, id: &str) -> Option<&'a str> {
    get_edge(graph, id, ISA_FIELD).and_then(get_uuid)
}

fn flatten_list(graph: &Graph, list_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = list_id.to_string();
    let mut seen = std::collections::HashSet::new();

    while seen.insert(current.clone()) {
        match get_isa(graph, &current) {
            Some(isa) if isa == CONS_T => {
                if let Some(head_id) = get_edge(graph, &current, HEAD_FIELD).and_then(get_uuid) {
                    result.push(head_id.to_string());
                }
                match get_edge(graph, &current, TAIL_FIELD).and_then(get_uuid) {
                    Some(tail_id) => current = tail_id.to_string(),
                    None => break,
                }
            }
            Some(isa) if isa == EMPTY_T => break,
            _ => break,
        }
    }
    result
}

fn sanitize_ident(name: &str) -> String {
    let s = name.replace(['-', ' '], "_");
    match s.as_str() {
        "type" | "self" | "Self" | "fn" | "let" | "mut" | "ref" | "mod" | "use" |
        "pub" | "impl" | "trait" | "struct" | "enum" | "const" | "static" |
        "move" | "return" | "if" | "else" | "match" | "for" | "while" | "loop" |
        "break" | "continue" | "where" | "async" | "await" | "dyn" | "crate" |
        "super" | "in" | "as" | "unsafe" | "extern" => format!("{}_", s),
        _ => s
    }
}

fn capitalize(s: &str) -> String {
    s.chars().next().map(|c| c.to_uppercase().to_string() + &s[1..]).unwrap_or_default()
}

#[derive(Debug, Clone)]
enum ResolvedType {
    String,
    Number,
    Record(String),  // name
    List(Box<ResolvedType>),
    Untyped,
}

fn resolve_type(graph: &Graph, type_id: &str) -> ResolvedType {
    if type_id == STRING_T {
        return ResolvedType::String;
    }
    if type_id == NUMBER_T {
        return ResolvedType::Number;
    }

    match get_isa(graph, type_id) {
        Some(isa) if isa == RECORD_T => {
            let name = get_name(graph, type_id).unwrap_or_default();
            ResolvedType::Record(name)
        }
        Some(isa) if isa == TYPE_T => {
            let name = get_name(graph, type_id).unwrap_or_default();
            match name.as_str() {
                "string" => ResolvedType::String,
                "number" => ResolvedType::Number,
                _ => {
                    if let Some(body_id) = get_edge(graph, type_id, BODY_FIELD).and_then(get_uuid) {
                        let body_isa = get_isa(graph, body_id);
                        if body_isa == Some(RECORD_T) {
                            ResolvedType::Record(name)
                        } else {
                            resolve_type(graph, body_id)
                        }
                    } else {
                        ResolvedType::Untyped
                    }
                }
            }
        }
        Some(isa) if isa == APPLY_T => {
            let base_id = get_edge(graph, type_id, BASE_FIELD).and_then(get_uuid);

            // Only handle List<T>, other applies are untyped for now
            if base_id == Some(LIST_T) {
                let args: Vec<ResolvedType> = get_edge(graph, type_id, ARGS_FIELD)
                    .and_then(get_uuid)
                    .map(|list_id| {
                        flatten_list(graph, list_id)
                            .into_iter()
                            .map(|a| resolve_type(graph, &a))
                            .collect()
                    })
                    .unwrap_or_default();

                if args.len() == 1 {
                    ResolvedType::List(Box::new(args.into_iter().next().unwrap()))
                } else {
                    ResolvedType::Untyped
                }
            } else {
                ResolvedType::Untyped
            }
        }
        _ => ResolvedType::Untyped,
    }
}

fn get_record_fields(graph: &Graph, record_id: &str) -> Vec<(String, String, ResolvedType)> {
    get_edge(graph, record_id, FIELDS_FIELD)
        .and_then(get_uuid)
        .map(|list_id| {
            flatten_list(graph, list_id)
                .into_iter()
                .filter_map(|field_id| {
                    let name = get_name(graph, &field_id)?;
                    let field_type = get_edge(graph, &field_id, TYPE_FIELD)
                        .and_then(get_uuid)
                        .map(|t| resolve_type(graph, t))
                        .unwrap_or(ResolvedType::Untyped);
                    Some((field_id, name, field_type))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn generate_accessor(field_id: &str, field_name: &str, field_type: &ResolvedType) -> TokenStream2 {
    if field_name.is_empty() {
        panic!("Empty field_name for field_id={}", field_id);
    }
    let method_name = format_ident!("{}", sanitize_ident(field_name));
    let field_uuid_str = field_id.to_string();

    match field_type {
        ResolvedType::String => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<std::string::String> {
                match gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))? {
                    crate::graph::Id::String(s) => Some(s.clone()),
                    _ => None,
                }
            }
        },
        ResolvedType::Number => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<f64> {
                match gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))? {
                    crate::graph::Id::Number(n) => Some(n.0),
                    _ => None,
                }
            }
        },
        ResolvedType::Record(name) => {
            let wrapper_name = format_ident!("{}", capitalize(name));
            quote! {
                pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#wrapper_name> {
                    let id = gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))?;
                    Some(#wrapper_name::wrap(id.clone()))
                }
            }
        },
        ResolvedType::List(element_type) => {
            let head_field_str = HEAD_FIELD;
            let tail_field_str = TAIL_FIELD;
            let cons_t_str = CONS_T;
            let isa_field_str = ISA_FIELD;

            let (return_type, element_conversion) = match element_type.as_ref() {
                ResolvedType::String => (
                    quote! { impl Iterator<Item = std::string::String> + 'a },
                    quote! {
                        .filter_map(|id| match id {
                            crate::graph::Id::String(s) => Some(s.clone()),
                            _ => None,
                        })
                    }
                ),
                ResolvedType::Number => (
                    quote! { impl Iterator<Item = f64> + 'a },
                    quote! {
                        .filter_map(|id| match id {
                            crate::graph::Id::Number(n) => Some(n.0),
                            _ => None,
                        })
                    }
                ),
                ResolvedType::Record(elem_name) => {
                    let wrapper = format_ident!("{}", capitalize(elem_name));
                    (
                        quote! { impl Iterator<Item = #wrapper> + 'a },
                        quote! { .map(|id| #wrapper::wrap(id.clone())) }
                    )
                },
                _ => (
                    quote! { impl Iterator<Item = crate::graph::Id> + 'a },
                    quote! { .cloned() }
                ),
            };

            quote! {
                pub fn #method_name<'a>(&self, gid: &'a impl crate::graph::Gid) -> #return_type {
                    crate::graph::ListIter::new(
                        gid,
                        gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap())),
                        crate::graph::Id::Uuid(uuid::Uuid::parse_str(#isa_field_str).unwrap()),
                        crate::graph::Id::Uuid(uuid::Uuid::parse_str(#cons_t_str).unwrap()),
                        crate::graph::Id::Uuid(uuid::Uuid::parse_str(#head_field_str).unwrap()),
                        crate::graph::Id::Uuid(uuid::Uuid::parse_str(#tail_field_str).unwrap()),
                    )#element_conversion
                }
            }
        },
        _ => quote! {
            pub fn #method_name<'a>(&self, gid: &'a impl crate::graph::Gid) -> Option<&'a crate::graph::Id> {
                gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))
            }
        },
    }
}

fn generate_wrapper(graph: &Graph, type_id: &str, body_id: &str, type_name: &str, extra_constants: &[(String, String)]) -> TokenStream2 {
    let fields = get_record_fields(graph, body_id);
    if type_name.is_empty() {
        panic!("Empty type_name for type_id={}, body_id={}", type_id, body_id);
    }
    let struct_name = format_ident!("{}", capitalize(type_name));
    let type_uuid: Uuid = type_id.parse().expect("invalid uuid");
    let type_uuid_str = type_uuid.to_string();

    let field_methods: Vec<TokenStream2> = fields
        .iter()
        .map(|(field_id, field_name, field_type)| {
            generate_accessor(field_id, field_name, field_type)
        })
        .collect();

    let extra_consts: Vec<TokenStream2> = extra_constants
        .iter()
        .map(|(name, uuid)| {
            let const_name = format_ident!("{}", name.to_uppercase());
            quote! { pub const #const_name: &'static str = #uuid; }
        })
        .collect();

    quote! {
        #[derive(Clone, Debug)]
        pub struct #struct_name(pub crate::graph::Id);

        impl #struct_name {
            pub const TYPE_ID: &'static str = #type_uuid_str;
            #(#extra_consts)*

            pub fn wrap(id: crate::graph::Id) -> Self {
                Self(id)
            }

            pub fn id(&self) -> &crate::graph::Id {
                &self.0
            }

            #(#field_methods)*
        }
    }
}

#[proc_macro]
pub fn generate_semantics(input: TokenStream) -> TokenStream {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");

    let path = if input.is_empty() {
        format!("{}/semantics.progred", manifest_dir)
    } else {
        let lit: syn::LitStr = syn::parse(input).expect("expected string literal path");
        let rel_path = lit.value();
        if rel_path.starts_with('/') {
            rel_path
        } else {
            format!("{}/{}", manifest_dir, rel_path)
        }
    };

    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));

    let json: Value = serde_json::from_str(&contents)
        .expect("failed to parse semantics file as JSON");

    let graph: Graph = serde_json::from_value(json.get("graph").expect("missing graph field").clone())
        .expect("failed to parse graph");

    let roots: Vec<Value> = serde_json::from_value(json.get("roots").expect("missing roots field").clone())
        .expect("failed to parse roots");

    let field_constants: Vec<(String, String)> = vec![
        ("name".to_string(), NAME_FIELD.to_string()),
        ("isa".to_string(), ISA_FIELD.to_string()),
        ("body".to_string(), BODY_FIELD.to_string()),
        ("params".to_string(), PARAMS_FIELD.to_string()),
        ("base".to_string(), BASE_FIELD.to_string()),
        ("args".to_string(), ARGS_FIELD.to_string()),
        ("variants".to_string(), VARIANTS_FIELD.to_string()),
        ("fields".to_string(), FIELDS_FIELD.to_string()),
        ("type_".to_string(), TYPE_FIELD.to_string()),
        ("head".to_string(), HEAD_FIELD.to_string()),
        ("tail".to_string(), TAIL_FIELD.to_string()),
    ];

    let wrappers: Vec<TokenStream2> = roots
        .iter()
        .filter_map(|root| {
            let id = get_uuid(root)?;
            let isa = get_isa(&graph, id)?;
            let name = get_name(&graph, id)?;
            let extra = if name == "field" { &field_constants } else { &vec![] };

            // Type wrappers: type { body: record { fields } }
            if isa == TYPE_T {
                let body_id = get_edge(&graph, id, BODY_FIELD).and_then(get_uuid)?;
                if get_isa(&graph, body_id) == Some(RECORD_T) {
                    return Some(generate_wrapper(&graph, id, body_id, &name, extra));
                }
            }
            // Direct records (legacy)
            if isa == RECORD_T {
                return Some(generate_wrapper(&graph, id, id, &name, extra));
            }
            None
        })
        .collect();

    let string_t_str = STRING_T;
    let number_t_str = NUMBER_T;
    let list_t_str = LIST_T;
    let type_expr_t_str = TYPE_EXPR_T;
    let cons_t_str = CONS_T;
    let empty_t_str = EMPTY_T;

    let output = quote! {
        pub mod semantics {
            pub const STRING_TYPE: &str = #string_t_str;
            pub const NUMBER_TYPE: &str = #number_t_str;
            pub const LIST_TYPE: &str = #list_t_str;
            pub const TYPE_EXPR: &str = #type_expr_t_str;
            pub const CONS_TYPE: &str = #cons_t_str;
            pub const EMPTY_TYPE: &str = #empty_t_str;

            #(#wrappers)*
        }
    };

    output.into()
}
