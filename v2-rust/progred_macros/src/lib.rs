use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

const NAME_FIELD: &str = "38f3aabf-d3a1-4c99-80d6-3de62afec12e";
const ISA_FIELD: &str = "a567ccd2-129d-4e85-a321-537a5a3857fb";
const FIELDS_FIELD: &str = "b6ad2f0c-7024-435f-9e1f-3be3a736b973";
const HEAD_FIELD: &str = "7e5593fb-b17c-4995-b8f4-37496c718ef2";
const TAIL_FIELD: &str = "14c2b086-2f99-4ef1-9d66-2fd5c4e94116";
const ENUM_TYPE: &str = "8b0e756a-1b23-44cc-be19-14fe2fcfa3a5";
const CONS_VARIANT: &str = "6026b370-d464-42bf-b660-ed3af2464463";
const EMPTY_VARIANT: &str = "024cee20-6439-404e-aa77-a8aeb7e83b06";

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

fn get_fields(graph: &Graph, type_id: &str) -> Vec<(String, String)> {
    get_edge(graph, type_id, FIELDS_FIELD)
        .and_then(get_uuid)
        .map(|list_id| {
            flatten_list(graph, list_id)
                .into_iter()
                .filter_map(|field_id| {
                    let name = get_name(graph, &field_id)?;
                    Some((field_id, name))
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
        .map(|(field_id, field_name)| {
            let method_name = format_ident!("{}", sanitize_ident(field_name));
            let field_uuid: Uuid = field_id.parse().expect("invalid uuid");
            let field_uuid_str = field_uuid.to_string();

            quote! {
                pub fn #method_name<'a>(&self, gid: &'a impl crate::graph::Gid) -> Option<&'a crate::graph::Id> {
                    gid.get(&self.0, &crate::graph::Id::Uuid(uuid::Uuid::parse_str(#field_uuid_str).unwrap()))
                }
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
