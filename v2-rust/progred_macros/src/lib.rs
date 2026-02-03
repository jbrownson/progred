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
const BASE_FIELD: &str = "4c81a576-7858-46ed-8246-12158e0c4a7a";
const ARGS_FIELD: &str = "1d43868e-5873-45dc-a3b1-5f8cef0b410e";
const FIELDS_FIELD: &str = "f355ead0-6bfb-450e-a729-1735c3950a0e";
const TYPE_FIELD: &str = "7f328ac9-de07-45a5-86b3-021b2711747a";
const HEAD_FIELD: &str = "83f724d9-1008-43e3-807c-f66adc7d774f";
const TAIL_FIELD: &str = "cffc46b8-2388-4197-ad87-7e7ef446accb";

// Type construct IDs
const TYPE_T: &str = "b7185815-d553-4fbf-9025-d88643a7ba6a";
const APPLY_T: &str = "d6ad0273-886d-4e83-9ed7-e4fe5fe2f4e8";
const RECORD_T: &str = "4ae8a1de-f7f5-4733-b6ca-0e01862635e6";
const FIELD_T: &str = "c9bdef1a-c9b0-4927-a881-5cf7fa54bb9b";

// Primitive type IDs
const STRING_T: &str = "355e01b2-4cbd-413a-a89d-e05fba7c577d";
const NUMBER_T: &str = "d4325076-d33e-4fc8-b3f6-5ba85385a352";

// List bootstrap IDs
const CONS_T: &str = "264e80b0-a8e1-4fc2-a3bd-6c10ff177261";
const EMPTY_T: &str = "1dff7f8a-1d83-41fd-af2d-087b753ed008";

// Auxiliary type IDs
const LIST_T: &str = "c80d8e16-dbf4-4a83-80fd-38b2de25688f";

type Graph = HashMap<String, HashMap<String, Value>>;
type Result<T> = std::result::Result<T, String>;

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
            Some(CONS_T) => {
                if let Some(head_id) = get_edge(graph, &current, HEAD_FIELD).and_then(get_uuid) {
                    result.push(head_id.to_string());
                }
                match get_edge(graph, &current, TAIL_FIELD).and_then(get_uuid) {
                    Some(tail_id) => current = tail_id.to_string(),
                    None => break,
                }
            }
            Some(EMPTY_T) => break,
            _ => break,
        }
    }
    result
}

const RESERVED: &[&str] = &[
    "type", "self", "Self", "fn", "let", "mut", "ref", "mod", "use",
    "pub", "impl", "trait", "struct", "enum", "const", "static",
    "move", "return", "if", "else", "match", "for", "while", "loop",
    "break", "continue", "where", "async", "await", "dyn", "crate",
    "super", "in", "as", "unsafe", "extern",
];

fn validate_ident(result: String, original: &str) -> Result<String> {
    if result.is_empty() {
        return Err(format!("Name {:?} produces empty identifier", original));
    }
    if result.chars().next().unwrap().is_ascii_digit() {
        return Err(format!("Name {:?} produces identifier starting with digit: {}", original, result));
    }
    if let Some(bad) = result.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '_') {
        return Err(format!("Name {:?} produces {:?} with invalid character {:?}", original, result, bad));
    }
    Ok(result)
}

fn escape_reserved(s: String) -> String {
    if RESERVED.contains(&s.as_str()) { format!("{}_", s) } else { s }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn rust_method_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(name.replace(['-', ' '], "_")), name)
}

fn rust_type_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(name.split(['-', ' ', '_']).map(capitalize).collect()), name)
}

fn rust_const_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(name.replace(['-', ' '], "_")).to_uppercase(), name)
}

#[derive(Debug, Clone)]
enum ResolvedType {
    String,
    Number,
    Record { rust_name: String },
    List(Box<ResolvedType>),
    Untyped,
}

fn resolve_type(graph: &Graph, type_id: &str) -> Result<ResolvedType> {
    match type_id {
        STRING_T => Ok(ResolvedType::String),
        NUMBER_T => Ok(ResolvedType::Number),
        _ => match get_isa(graph, type_id) {
            Some(RECORD_T) => get_name(graph, type_id)
                .ok_or_else(|| format!("Record {} has no name", type_id))
                .and_then(|name| rust_type_name(&name).map(|rust_name| ResolvedType::Record { rust_name })),
            Some(TYPE_T) => {
                match get_edge(graph, type_id, BODY_FIELD).and_then(get_uuid) {
                    Some(body_id) if get_isa(graph, body_id) == Some(RECORD_T) =>
                        get_name(graph, type_id)
                            .ok_or_else(|| format!("Type {} has no name", type_id))
                            .and_then(|name| rust_type_name(&name).map(|rust_name| ResolvedType::Record { rust_name })),
                    Some(body_id) => resolve_type(graph, body_id),
                    None => Ok(ResolvedType::Untyped),
                }
            }
            Some(APPLY_T) => {
                match get_edge(graph, type_id, BASE_FIELD).and_then(get_uuid) {
                    Some(LIST_T) => {
                        let args_list_id = get_edge(graph, type_id, ARGS_FIELD)
                            .and_then(get_uuid)
                            .ok_or_else(|| format!("List type {} has no args", type_id))?;
                        let args: Vec<ResolvedType> = flatten_list(graph, args_list_id)
                            .into_iter()
                            .map(|a| resolve_type(graph, &a))
                            .collect::<Result<_>>()?;

                        match args.as_slice() {
                            [single] => Ok(ResolvedType::List(Box::new(single.clone()))),
                            _ => Err(format!("List type {} has {} args, expected 1", type_id, args.len())),
                        }
                    }
                    _ => Ok(ResolvedType::Untyped),
                }
            }
            _ => Ok(ResolvedType::Untyped),
        }
    }
}

fn get_record_field_ids(graph: &Graph, record_id: &str) -> Result<Vec<String>> {
    let fields_list_id = get_edge(graph, record_id, FIELDS_FIELD)
        .and_then(get_uuid)
        .ok_or_else(|| format!("Record {} has no fields list", record_id))?;
    Ok(flatten_list(graph, fields_list_id))
}

fn uuid_expr(uuid_str: &str) -> TokenStream2 {
    let bytes = uuid_str.parse::<Uuid>().expect("invalid uuid").as_bytes().clone();
    quote! { uuid::Uuid::from_bytes([#(#bytes),*]) }
}

fn generate_accessor(graph: &Graph, field_id: &str) -> Result<TokenStream2> {
    let field_name = get_name(graph, field_id)
        .ok_or_else(|| format!("Field {} has no name", field_id))?;
    let field_type = match get_edge(graph, field_id, TYPE_FIELD).and_then(get_uuid) {
        Some(t) => resolve_type(graph, t)?,
        None => ResolvedType::Untyped,
    };
    let method_name = format_ident!("{}", rust_method_name(&field_name)?);
    let field_uuid = uuid_expr(field_id);

    Ok(match field_type {
        ResolvedType::String => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<std::string::String> {
                match gid.get(&self.0, &crate::graph::Id::Uuid(#field_uuid))? {
                    crate::graph::Id::String(s) => Some(s.clone()),
                    _ => None,
                }
            }
        },
        ResolvedType::Number => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<f64> {
                match gid.get(&self.0, &crate::graph::Id::Uuid(#field_uuid))? {
                    crate::graph::Id::Number(n) => Some(n.0),
                    _ => None,
                }
            }
        },
        ResolvedType::Record { rust_name } => {
            let wrapper_name = format_ident!("{}", rust_name);
            quote! {
                pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#wrapper_name> {
                    let id = gid.get(&self.0, &crate::graph::Id::Uuid(#field_uuid))?;
                    Some(#wrapper_name::wrap(id.clone()))
                }
            }
        },
        ResolvedType::List(element_type) => {
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
                ResolvedType::Record { rust_name } => {
                    let wrapper = format_ident!("{}", rust_name);
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
                    crate::list_iter::ListIter::new(
                        gid,
                        gid.get(&self.0, &crate::graph::Id::Uuid(#field_uuid)),
                    )#element_conversion
                }
            }
        },
        ResolvedType::Untyped => quote! {
            pub fn #method_name<'a>(&self, gid: &'a impl crate::graph::Gid) -> Option<&'a crate::graph::Id> {
                gid.get(&self.0, &crate::graph::Id::Uuid(#field_uuid))
            }
        },
    })
}

fn generate_wrapper(graph: &Graph, type_id: &str, body_id: &str, type_name: &str) -> Result<TokenStream2> {
    let field_ids = get_record_field_ids(graph, body_id)?;
    let struct_name = format_ident!("{}", rust_type_name(type_name)?);
    let type_uuid: Uuid = type_id.parse()
        .map_err(|e| format!("Invalid uuid {}: {}", type_id, e))?;
    let type_uuid_str = type_uuid.to_string();

    let field_methods: Vec<TokenStream2> = field_ids
        .iter()
        .map(|field_id| generate_accessor(graph, field_id))
        .collect::<Result<_>>()?;

    Ok(quote! {
        #[derive(Clone, Debug)]
        pub struct #struct_name(pub crate::graph::Id);

        impl #struct_name {
            pub const TYPE_ID: &'static str = #type_uuid_str;

            pub fn wrap(id: crate::graph::Id) -> Self {
                Self(id)
            }

            pub fn id(&self) -> &crate::graph::Id {
                &self.0
            }

            #(#field_methods)*
        }
    })
}

fn load_semantics(input: TokenStream) -> Result<(Graph, Vec<Value>)> {
    let path = format!("{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "CARGO_MANIFEST_DIR not set")?,
        syn::parse::<syn::LitStr>(input).map_err(|e| format!("expected path string: {}", e))?.value());

    let json: Value = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {}", path, e))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("failed to parse JSON: {}", e)))?;

    Ok((
        serde_json::from_value(json.get("graph").ok_or("missing graph field")?.clone())
            .map_err(|e| format!("failed to parse graph: {}", e))?,
        serde_json::from_value(json.get("roots").ok_or("missing roots field")?.clone())
            .map_err(|e| format!("failed to parse roots: {}", e))?,
    ))
}

fn generate_semantics_impl(input: TokenStream) -> Result<TokenStream2> {
    let (graph, roots) = load_semantics(input)?;

    let wrappers: Vec<TokenStream2> = roots.iter()
        .filter_map(|root| {
            let id = get_uuid(root)?;
            let isa = get_isa(&graph, id)?;
            let name = get_name(&graph, id)?;
            match isa {
                TYPE_T => {
                    let body_id = get_edge(&graph, id, BODY_FIELD).and_then(get_uuid)?;
                    (get_isa(&graph, body_id) == Some(RECORD_T))
                        .then(|| generate_wrapper(&graph, id, body_id, &name))
                }
                RECORD_T => Some(generate_wrapper(&graph, id, id, &name)),
                _ => None,
            }
        })
        .collect::<Result<_>>()?;

    let field_constants: Vec<TokenStream2> = graph.keys()
        .filter(|id| get_isa(&graph, id) == Some(FIELD_T))
        .filter_map(|id| get_name(&graph, id).map(|name| (id, name)))
        .map(|(id, name)| {
            let const_name = format_ident!("{}", rust_const_name(&name)?);
            let uuid_str = id.to_string();
            Ok(quote! { pub const #const_name: &str = #uuid_str; })
        })
        .collect::<Result<_>>()?;

    let type_constants: Vec<TokenStream2> = graph.keys()
        .filter(|id| get_isa(&graph, id) == Some(TYPE_T))
        .filter_map(|id| get_name(&graph, id).map(|name| (id, name)))
        .map(|(id, name)| {
            let const_name = format_ident!("{}_TYPE", rust_const_name(&name)?);
            let uuid_str = id.to_string();
            Ok(quote! { pub const #const_name: &str = #uuid_str; })
        })
        .collect::<Result<_>>()?;

    Ok(quote! {
        pub mod semantics {
            #(#type_constants)*
            #(#field_constants)*
            #(#wrappers)*
        }
    })
}

#[proc_macro]
pub fn generate_semantics(input: TokenStream) -> TokenStream {
    match generate_semantics_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(msg) => {
            let error = syn::Error::new(proc_macro2::Span::call_site(), msg);
            error.to_compile_error().into()
        }
    }
}
