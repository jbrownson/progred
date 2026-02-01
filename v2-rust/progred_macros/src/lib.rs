use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

const NAME_FIELD: &str = "38f3aabf-d3a1-4c99-80d6-3de62afec12e";
const ISA_FIELD: &str = "a567ccd2-129d-4e85-a321-537a5a3857fb";
const FIELDS_FIELD: &str = "b6ad2f0c-7024-435f-9e1f-3be3a736b973";
const TYPE_FIELD: &str = "3eb2af75-6c78-424a-80e0-b792a37bd1ab";
const HEAD_FIELD: &str = "7e5593fb-b17c-4995-b8f4-37496c718ef2";
const TAIL_FIELD: &str = "14c2b086-2f99-4ef1-9d66-2fd5c4e94116";
const ENUM_TYPE: &str = "8b0e756a-1b23-44cc-be19-14fe2fcfa3a5";
const CONS_VARIANT: &str = "6026b370-d464-42bf-b660-ed3af2464463";
const EMPTY_VARIANT: &str = "024cee20-6439-404e-aa77-a8aeb7e83b06";
const STRING_TYPE: &str = "cd44ea12-0e7f-4775-8bfa-557bccc84ec5";
const NUMBER_TYPE: &str = "099d2d01-9fe8-466c-8978-1989cfe0b032";
const LIST_TYPE: &str = "f3d0577f-5a71-42a2-90e3-f083fad9a129";

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

fn get_type<'a>(graph: &'a Graph, id: &str) -> Option<&'a str> {
    get_edge(graph, id, TYPE_FIELD).and_then(get_uuid)
}

enum FieldType {
    String,
    Number,
    Enum(String, String), // (type_id, type_name)
    List(Box<FieldType>), // element type
    Untyped,
}

fn flatten_list(graph: &Graph, list_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = list_id.to_string();
    let mut seen = std::collections::HashSet::new();

    while seen.insert(current.clone()) {
        let isa = get_isa(graph, &current);
        if isa == Some(CONS_VARIANT) {
            if let Some(head_id) = get_edge(graph, &current, HEAD_FIELD).and_then(get_uuid) {
                result.push(head_id.to_string());
            }
            if let Some(tail_id) = get_edge(graph, &current, TAIL_FIELD).and_then(get_uuid) {
                current = tail_id.to_string();
            } else {
                break;
            }
        } else if isa == Some(EMPTY_VARIANT) {
            break;
        } else {
            break;
        }
    }
    result
}

fn resolve_type(graph: &Graph, type_id: &str) -> FieldType {
    if type_id == STRING_TYPE {
        FieldType::String
    } else if type_id == NUMBER_TYPE {
        FieldType::Number
    } else if get_isa(graph, type_id) == Some(LIST_TYPE) {
        // It's a ListType - get the element type
        let element_type = get_type(graph, type_id)
            .map(|elem_type_id| resolve_type(graph, elem_type_id))
            .unwrap_or(FieldType::Untyped);
        FieldType::List(Box::new(element_type))
    } else if get_isa(graph, type_id) == Some(ENUM_TYPE) {
        if let Some(type_name) = get_name(graph, type_id) {
            return FieldType::Enum(type_id.to_string(), type_name);
        }
        FieldType::Untyped
    } else {
        FieldType::Untyped
    }
}

fn resolve_field_type(graph: &Graph, field_id: &str) -> FieldType {
    match get_type(graph, field_id) {
        Some(type_id) => resolve_type(graph, type_id),
        None => FieldType::Untyped,
    }
}

fn get_fields(graph: &Graph, type_id: &str) -> Vec<(String, String, FieldType)> {
    get_edge(graph, type_id, FIELDS_FIELD)
        .and_then(get_uuid)
        .map(|list_id| {
            flatten_list(graph, list_id)
                .into_iter()
                .filter_map(|field_id| {
                    let name = get_name(graph, &field_id)?;
                    let field_type = resolve_field_type(graph, &field_id);
                    Some((field_id, name, field_type))
                })
                .collect()
        })
        .unwrap_or_default()
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

fn generate_wrapper(graph: &Graph, type_id: &str, type_name: &str) -> TokenStream2 {
    let fields = get_fields(graph, type_id);
    let struct_name = format_ident!("{}", type_name.to_uppercase().chars().next().unwrap().to_string() + &type_name[1..]);
    let type_uuid: Uuid = type_id.parse().expect("invalid uuid");
    let type_uuid_str = type_uuid.to_string();

    let field_methods: Vec<TokenStream2> = fields
        .iter()
        .map(|(field_id, field_name, field_type)| {
            let method_name = format_ident!("{}", sanitize_ident(field_name));
            let field_uuid: Uuid = field_id.parse().expect("invalid uuid");
            let field_uuid_str = field_uuid.to_string();

            match field_type {
                FieldType::String => quote! {
                    pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<String> {
                        match gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))? {
                            crate::graph::Id::String(s) => Some(s.clone()),
                            _ => None,
                        }
                    }
                },
                FieldType::Number => quote! {
                    pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<f64> {
                        match gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))? {
                            crate::graph::Id::Number(n) => Some(n.0),
                            _ => None,
                        }
                    }
                },
                FieldType::Enum(_enum_type_id, enum_type_name) => {
                    let wrapper_name = format_ident!("{}", enum_type_name.to_uppercase().chars().next().unwrap().to_string() + &enum_type_name[1..]);
                    quote! {
                        pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#wrapper_name> {
                            let id = gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))?;
                            Some(#wrapper_name::wrap(id.clone()))
                        }
                    }
                },
                FieldType::List(element_type) => {
                    let head_field_str = HEAD_FIELD;
                    let tail_field_str = TAIL_FIELD;
                    let cons_variant_str = CONS_VARIANT;
                    let isa_field_str = ISA_FIELD;

                    let element_conversion = match element_type.as_ref() {
                        FieldType::String => quote! {
                            match head {
                                crate::graph::Id::String(s) => result.push(s.clone()),
                                _ => {}
                            }
                        },
                        FieldType::Number => quote! {
                            match head {
                                crate::graph::Id::Number(n) => result.push(n.0),
                                _ => {}
                            }
                        },
                        FieldType::Enum(_, enum_name) => {
                            let elem_wrapper = format_ident!("{}", enum_name.to_uppercase().chars().next().unwrap().to_string() + &enum_name[1..]);
                            quote! {
                                result.push(#elem_wrapper::wrap(head.clone()));
                            }
                        },
                        FieldType::List(_) => quote! {
                            result.push(head.clone());
                        },
                        FieldType::Untyped => quote! {
                            result.push(head.clone());
                        },
                    };

                    let return_type = match element_type.as_ref() {
                        FieldType::String => quote! { Vec<String> },
                        FieldType::Number => quote! { Vec<f64> },
                        FieldType::Enum(_, enum_name) => {
                            let elem_wrapper = format_ident!("{}", enum_name.to_uppercase().chars().next().unwrap().to_string() + &enum_name[1..]);
                            quote! { Vec<#elem_wrapper> }
                        },
                        FieldType::List(_) | FieldType::Untyped => quote! { Vec<crate::graph::Id> },
                    };

                    quote! {
                        pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> #return_type {
                            let head_field = crate::graph::Id::Uuid(uuid::Uuid::parse_str(#head_field_str).unwrap());
                            let tail_field = crate::graph::Id::Uuid(uuid::Uuid::parse_str(#tail_field_str).unwrap());
                            let cons_variant = crate::graph::Id::Uuid(uuid::Uuid::parse_str(#cons_variant_str).unwrap());
                            let isa_field = crate::graph::Id::Uuid(uuid::Uuid::parse_str(#isa_field_str).unwrap());

                            let mut result = Vec::new();
                            let Some(mut current) = gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap())) else {
                                return result;
                            };

                            let mut seen = std::collections::HashSet::new();
                            while gid.get(current, &isa_field) == Some(&cons_variant) {
                                if !seen.insert(current.clone()) {
                                    break;
                                }
                                if let Some(head) = gid.get(current, &head_field) {
                                    #element_conversion
                                }
                                match gid.get(current, &tail_field) {
                                    Some(tail) => current = tail,
                                    None => break,
                                }
                            }
                            result
                        }
                    }
                },
                FieldType::Untyped => quote! {
                    pub fn #method_name<'a>(&self, gid: &'a impl crate::graph::Gid) -> Option<&'a crate::graph::Id> {
                        gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))
                    }
                },
            }
        })
        .collect();

    quote! {
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
    }
}

#[proc_macro]
pub fn generate_semantics(input: TokenStream) -> TokenStream {
    let path = if input.is_empty() {
        std::env::var("CARGO_MANIFEST_DIR")
            .map(|dir| format!("{}/semantics.progred", dir))
            .expect("CARGO_MANIFEST_DIR not set")
    } else {
        let lit: syn::LitStr = syn::parse(input).expect("expected string literal path");
        lit.value()
    };

    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));

    let json: Value = serde_json::from_str(&contents)
        .expect("failed to parse semantics file as JSON");

    let graph: Graph = serde_json::from_value(json.get("graph").expect("missing graph field").clone())
        .expect("failed to parse graph");

    let roots: Vec<Value> = serde_json::from_value(json.get("roots").expect("missing roots field").clone())
        .expect("failed to parse roots");

    let types: Vec<(String, String)> = roots
        .iter()
        .filter_map(|root| {
            let id = get_uuid(root)?;
            (get_isa(&graph, id) == Some(ENUM_TYPE)).then_some(())?;
            let name = get_name(&graph, id)?;
            Some((id.to_string(), name))
        })
        .collect();

    let wrappers: Vec<TokenStream2> = types
        .iter()
        .map(|(id, name)| generate_wrapper(&graph, id, name))
        .collect();

    let output = quote! {
        pub mod semantics {
            #(#wrappers)*
        }
    };

    output.into()
}
