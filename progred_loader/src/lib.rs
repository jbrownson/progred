use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use uuid::Uuid;
use progred_graph::{Id, Gid, MutGid};

type Result<T> = std::result::Result<T, String>;

fn load_json(input: TokenStream) -> Result<(String, MutGid, Vec<Id>)> {
    let relative_path = syn::parse::<syn::LitStr>(input)
        .map_err(|e| format!("expected path string: {}", e))?
        .value();
    let full_path = format!("{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "CARGO_MANIFEST_DIR not set")?,
        relative_path);

    let json: serde_json::Value = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("failed to read {}: {}", full_path, e))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("failed to parse JSON: {}", e)))?;

    let gid: MutGid = serde_json::from_value(json.get("graph").ok_or("missing graph field")?.clone())
        .map_err(|e| format!("failed to parse graph: {}", e))?;

    let roots: Vec<Id> = serde_json::from_value(json.get("roots").ok_or("missing roots field")?.clone())
        .map_err(|e| format!("failed to parse roots: {}", e))?;

    Ok((relative_path, gid, roots))
}

fn uuid_expr(uuid: &Uuid) -> TokenStream2 {
    let bytes = *uuid.as_bytes();
    quote! { uuid::Uuid::from_bytes([#(#bytes),*]) }
}

fn id_expr(id: &Id) -> TokenStream2 {
    match id {
        Id::Uuid(uuid) => {
            let uuid_tokens = uuid_expr(uuid);
            quote! { crate::graph::Id::Uuid(#uuid_tokens) }
        }
        Id::String(s) => quote! { crate::graph::Id::String(String::from(#s)) },
        Id::Number(n) => {
            let v = n.0;
            quote! { crate::graph::Id::Number(ordered_float::OrderedFloat(#v)) }
        }
    }
}

fn load_document_impl(input: TokenStream) -> Result<TokenStream2> {
    let (path, gid, roots) = load_json(input)?;

    let set_calls: Vec<TokenStream2> = gid.entities()
        .flat_map(|uuid| {
            let entity_expr = uuid_expr(uuid);
            let entity_id = Id::Uuid(*uuid);
            gid.edges(&entity_id).into_iter().flat_map(move |edges| {
                let entity_expr = entity_expr.clone();
                edges.iter().map(move |(label, value)| {
                    let label_expr = id_expr(label);
                    let value_expr = id_expr(value);
                    let entity_expr = entity_expr.clone();
                    quote! { gid.set(#entity_expr, #label_expr, #value_expr); }
                })
            })
        })
        .collect();

    let root_exprs: Vec<TokenStream2> = roots.iter()
        .map(|id| {
            let id_tokens = id_expr(id);
            quote! { crate::path::RootSlot::new(#id_tokens) }
        })
        .collect();

    let include_path = format!("/{}", path);
    Ok(quote! {
        {
            const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
            let mut gid = crate::graph::MutGid::new();
            #(#set_calls)*
            let roots = vec![#(#root_exprs),*];
            crate::document::Document { gid, roots }
        }
    })
}

#[proc_macro]
pub fn load_document(input: TokenStream) -> TokenStream {
    match load_document_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(msg) => {
            let error = syn::Error::new(proc_macro2::Span::call_site(), msg);
            error.to_compile_error().into()
        }
    }
}
