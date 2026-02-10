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
const FORALL_T: &str = "2e3784a7-0542-44be-90de-9ca8f8a606a4";
const SUM_T: &str = "6847449f-a122-40f5-8880-1692d968d127";
const PARAMS_FIELD: &str = "10650b24-4876-4726-bd57-b06ce8fcf874";
const VARIANTS_FIELD: &str = "de15c5ce-f65c-4e32-a15e-94f37dbb7541";

// Primitive type IDs
const STRING_T: &str = "355e01b2-4cbd-413a-a89d-e05fba7c577d";
const NUMBER_T: &str = "d4325076-d33e-4fc8-b3f6-5ba85385a352";

// List bootstrap IDs
const CONS_T: &str = "264e80b0-a8e1-4fc2-a3bd-6c10ff177261";
const EMPTY_T: &str = "1dff7f8a-1d83-41fd-af2d-087b753ed008";


type Graph = HashMap<String, HashMap<String, Value>>;
type Result<T> = std::result::Result<T, String>;
type Substitutions = HashMap<String, ResolvedType>;

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

fn validate_ident(s: String) -> Result<String> {
    if s.is_empty() {
        return Err("Empty identifier".to_string());
    }
    if s.chars().next().unwrap().is_ascii_digit() {
        return Err(format!("Identifier {:?} starts with digit", s));
    }
    if let Some(bad) = s.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '_') {
        return Err(format!("Identifier {:?} contains invalid character {:?}", s, bad));
    }
    Ok(s)
}

fn escape_reserved(s: String) -> String {
    if RESERVED.contains(&s.as_str()) { format!("{}_", s) } else { s }
}

fn to_snake_case(s: &str) -> String {
    s.replace(['-', ' '], "_")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', ' ', '_']).map(capitalize).collect()
}

fn rust_method_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(to_snake_case(name)))
}

fn rust_type_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(to_pascal_case(name)))
}

fn rust_const_name(name: &str) -> Result<String> {
    validate_ident(escape_reserved(to_snake_case(name)).to_uppercase())
}

#[derive(Debug, Clone)]
enum ResolvedType {
    String,
    Number,
    Record { rust_name: String },
    Generic { rust_name: String, args: Vec<ResolvedType> },
    TypeParam { rust_name: String, converter_name: String },
}

fn get_forall_params(graph: &Graph, type_id: &str) -> Option<Vec<String>> {
    let body_id = get_edge(graph, type_id, BODY_FIELD).and_then(get_uuid)?;
    if get_isa(graph, body_id) == Some(FORALL_T) {
        let params_list_id = get_edge(graph, body_id, PARAMS_FIELD).and_then(get_uuid)?;
        Some(flatten_list(graph, params_list_id))
    } else {
        None
    }
}

fn resolve_type(graph: &Graph, type_id: &str, subs: &Substitutions) -> Result<ResolvedType> {
    if let Some(resolved) = subs.get(type_id) {
        return Ok(resolved.clone());
    }

    match type_id {
        STRING_T => Ok(ResolvedType::String),
        NUMBER_T => Ok(ResolvedType::Number),
        _ => match get_isa(graph, type_id) {
            Some(RECORD_T) => Err(format!("Bare record {} not wrapped in named type", type_id)),
            Some(SUM_T) => Err(format!("Bare sum {} not wrapped in named type", type_id)),
            Some(TYPE_T) => {
                match get_edge(graph, type_id, BODY_FIELD).and_then(get_uuid) {
                    Some(body_id) => {
                        let body_isa = get_isa(graph, body_id);
                        if body_isa == Some(RECORD_T) || body_isa == Some(SUM_T) {
                            get_name(graph, type_id)
                                .ok_or_else(|| format!("Type {} has no name", type_id))
                                .and_then(|name| rust_type_name(&name).map(|rust_name| ResolvedType::Record { rust_name }))
                        } else {
                            resolve_type(graph, body_id, subs)
                        }
                    }
                    None => Err(format!("Type {} has no body", type_id)),
                }
            }
            Some(APPLY_T) => {
                let base_id = get_edge(graph, type_id, BASE_FIELD)
                    .and_then(get_uuid)
                    .ok_or_else(|| format!("Apply {} has no base", type_id))?;
                let args_list_id = get_edge(graph, type_id, ARGS_FIELD)
                    .and_then(get_uuid)
                    .ok_or_else(|| format!("Apply {} has no args", type_id))?;
                let arg_ids = flatten_list(graph, args_list_id);

                if get_forall_params(graph, base_id).is_some() {
                    let rust_name = get_name(graph, base_id)
                        .ok_or_else(|| format!("Generic type {} has no name", base_id))
                        .and_then(|name| rust_type_name(&name))?;
                    let args = arg_ids
                        .iter()
                        .map(|a| resolve_type(graph, a, subs))
                        .collect::<Result<_>>()?;
                    Ok(ResolvedType::Generic { rust_name, args })
                } else {
                    Err(format!("Apply {} to non-generic type {}", type_id, base_id))
                }
            }
            _ => Err(format!("Unknown type structure: {}", type_id)),
        }
    }
}

fn get_record_field_ids(graph: &Graph, record_id: &str) -> Result<Vec<String>> {
    let fields_list_id = get_edge(graph, record_id, FIELDS_FIELD)
        .and_then(get_uuid)
        .ok_or_else(|| format!("Record {} has no fields list", record_id))?;
    Ok(flatten_list(graph, fields_list_id))
}

fn id_expr(uuid_str: &str) -> TokenStream2 {
    let bytes = *uuid_str.parse::<Uuid>().expect("invalid uuid").as_bytes();
    quote! { crate::graph::Id::Uuid(uuid::Uuid::from_bytes([#(#bytes),*])) }
}

fn generate_setter(graph: &Graph, field_id: &str, subs: &Substitutions, self_id: &TokenStream2) -> Result<TokenStream2> {
    let field_name = get_name(graph, field_id)
        .ok_or_else(|| format!("Field {} has no name", field_id))?;
    let field_type = match get_edge(graph, field_id, TYPE_FIELD).and_then(get_uuid) {
        Some(t) => resolve_type(graph, t, subs)?,
        None => return Err(format!("Field {} has no type", field_id)),
    };
    let method_name = format_ident!("set_{}", rust_method_name(&field_name)?);
    let field_id = id_expr(field_id);

    Ok(match field_type {
        ResolvedType::String => quote! {
            pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: impl Into<std::string::String>) {
                gid.set(#self_id.clone(), #field_id, crate::graph::Id::String(value.into()));
            }
        },
        ResolvedType::Number => quote! {
            pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: f64) {
                gid.set(#self_id.clone(), #field_id, crate::graph::Id::Number(ordered_float::OrderedFloat(value)));
            }
        },
        ResolvedType::Record { rust_name } => {
            let wrapper_name = format_ident!("{}", rust_name);
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &#wrapper_name) {
                    gid.set(#self_id.clone(), #field_id, value.id().clone());
                }
            }
        },
        ResolvedType::Generic { rust_name, args } => {
            let wrapper = format_ident!("{}", rust_name);
            let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &#wrapper<#(#arg_types),*>) {
                    gid.set(#self_id.clone(), #field_id, value.id().clone());
                }
            }
        },
        ResolvedType::TypeParam { .. } => {
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &crate::graph::Id) {
                    gid.set(#self_id.clone(), #field_id, value.clone());
                }
            }
        },
    })
}

fn generate_accessor(graph: &Graph, field_id: &str, subs: &Substitutions, self_id: &TokenStream2) -> Result<TokenStream2> {
    let field_name = get_name(graph, field_id)
        .ok_or_else(|| format!("Field {} has no name", field_id))?;
    let field_type = match get_edge(graph, field_id, TYPE_FIELD).and_then(get_uuid) {
        Some(t) => resolve_type(graph, t, subs)?,
        None => return Err(format!("Field {} has no type", field_id)),
    };
    let method_name = format_ident!("{}", rust_method_name(&field_name)?);
    let field_id = id_expr(field_id);

    Ok(match field_type {
        ResolvedType::String => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<std::string::String> {
                match gid.get(&#self_id, &#field_id)? {
                    crate::graph::Id::String(s) => Some(s.clone()),
                    _ => None,
                }
            }
        },
        ResolvedType::Number => quote! {
            pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<f64> {
                match gid.get(&#self_id, &#field_id)? {
                    crate::graph::Id::Number(n) => Some(n.0),
                    _ => None,
                }
            }
        },
        ResolvedType::Record { rust_name } => {
            let wrapper_name = format_ident!("{}", rust_name);
            quote! {
                pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#wrapper_name> {
                    let id = gid.get(&#self_id, &#field_id)?;
                    Some(#wrapper_name::wrap(id.clone()))
                }
            }
        },
        ResolvedType::Generic { rust_name, args } => {
            let wrapper = format_ident!("{}", rust_name);
            let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
            let converters: Vec<_> = args.iter().map(|arg| {
                match arg {
                    ResolvedType::TypeParam { converter_name, .. } => {
                        let conv = format_ident!("{}", converter_name);
                        quote! { self.#conv.clone() }
                    }
                    _ => {
                        let (_, converter) = resolved_type_to_rust(arg);
                        let conversion = converter(quote! { id });
                        quote! { std::rc::Rc::new(|id| Some(#conversion)) }
                    }
                }
            }).collect();
            quote! {
                pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#wrapper<#(#arg_types),*>> {
                    let id = gid.get(&#self_id, &#field_id)?;
                    Some(#wrapper::wrap(id.clone(), #(#converters),*))
                }
            }
        },
        ResolvedType::TypeParam { rust_name, converter_name } => {
            let type_ident = format_ident!("{}", rust_name);
            let conv = format_ident!("{}", converter_name);
            quote! {
                pub fn #method_name(&self, gid: &impl crate::graph::Gid) -> Option<#type_ident> {
                    let raw = gid.get(&#self_id, &#field_id)?;
                    (self.#conv)(raw)
                }
            }
        },
    })
}

fn get_sum_variant_ids(graph: &Graph, sum_id: &str) -> Result<Vec<String>> {
    let variants_list_id = get_edge(graph, sum_id, VARIANTS_FIELD)
        .and_then(get_uuid)
        .ok_or_else(|| format!("Sum {} has no variants list", sum_id))?;
    Ok(flatten_list(graph, variants_list_id))
}

struct TypeParam {
    id: String,
    rust_name: syn::Ident,
    converter_name: syn::Ident,
}

fn get_type_params(graph: &Graph, forall_id: &str) -> Result<Vec<TypeParam>> {
    let params_list_id = get_edge(graph, forall_id, PARAMS_FIELD)
        .and_then(get_uuid)
        .ok_or_else(|| format!("Forall {} has no params", forall_id))?;

    flatten_list(graph, params_list_id)
        .into_iter()
        .map(|param_id| {
            let name = get_name(graph, &param_id)
                .ok_or_else(|| format!("Type param {} has no name", param_id))?;
            let rust_name = format_ident!("{}", rust_type_name(&name)?);
            let converter_name = format_ident!("into_{}", rust_method_name(&name)?);
            Ok(TypeParam { id: param_id, rust_name, converter_name })
        })
        .collect()
}

fn resolved_type_to_rust(resolved: &ResolvedType) -> (TokenStream2, Box<dyn Fn(TokenStream2) -> TokenStream2>) {
    match resolved {
        ResolvedType::String => (
            quote! { std::string::String },
            Box::new(|id_expr| quote! {
                match #id_expr { crate::graph::Id::String(s) => s.clone(), _ => return None }
            })
        ),
        ResolvedType::Number => (
            quote! { f64 },
            Box::new(|id_expr| quote! {
                match #id_expr { crate::graph::Id::Number(n) => n.0, _ => return None }
            })
        ),
        ResolvedType::Record { rust_name } => {
            let wrapper = format_ident!("{}", rust_name);
            let name_clone = rust_name.clone();
            (
                quote! { #wrapper },
                Box::new(move |id_expr| {
                    let w = format_ident!("{}", name_clone);
                    quote! { #w::wrap(#id_expr.clone()) }
                })
            )
        },
        ResolvedType::Generic { rust_name, args } => {
            let wrapper = format_ident!("{}", rust_name);
            let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
            let converters: Vec<_> = args.iter().map(|arg| {
                match arg {
                    ResolvedType::TypeParam { converter_name, .. } => {
                        let conv = format_ident!("{}", converter_name);
                        quote! { self.#conv.clone() }
                    }
                    _ => {
                        let (_, converter) = resolved_type_to_rust(arg);
                        let conversion = converter(quote! { id });
                        quote! { std::rc::Rc::new(|id| Some(#conversion)) }
                    }
                }
            }).collect();
            let wrapper_clone = rust_name.clone();
            (
                quote! { #wrapper<#(#arg_types),*> },
                Box::new(move |id_expr| {
                    let w = format_ident!("{}", wrapper_clone);
                    quote! { #w::wrap(#id_expr.clone(), #(#converters),*) }
                })
            )
        },
        ResolvedType::TypeParam { rust_name, converter_name } => {
            let type_ident = format_ident!("{}", rust_name);
            let conv_name = converter_name.clone();
            (
                quote! { #type_ident },
                Box::new(move |id_expr| {
                    let conv = format_ident!("{}", conv_name);
                    quote! { (self.#conv)(#id_expr)? }
                })
            )
        },
    }
}

fn generate_sum_wrapper(graph: &Graph, type_id: &str, sum_id: &str, type_name: &str, type_params: &[TypeParam], subs: &Substitutions) -> Result<TokenStream2> {
    let full_subs: Substitutions = subs.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(type_params.iter().map(|tp| (tp.id.clone(), ResolvedType::TypeParam {
            rust_name: tp.rust_name.to_string(),
            converter_name: tp.converter_name.to_string(),
        })))
        .collect();

    let variant_ids = get_sum_variant_ids(graph, sum_id)?;
    let struct_name = format_ident!("{}", rust_type_name(type_name)?);
    let type_uuid = id_expr(type_id);
    let isa_uuid = id_expr(ISA_FIELD);

    struct VariantInfo {
        closure_name: syn::Ident,
        constructor_name: syn::Ident,
        variant_id: TokenStream2,
        field_types: Vec<TokenStream2>,
        constructor_field_types: Vec<TokenStream2>,
        field_names: Vec<syn::Ident>,
        field_ids: Vec<TokenStream2>,
        conversions: Vec<TokenStream2>,
        field_setters: Vec<TokenStream2>,
    }

    let variants: Vec<VariantInfo> = variant_ids.iter().map(|variant_id| {
        let variant_name = get_name(graph, variant_id)
            .ok_or_else(|| format!("Variant {} has no name", variant_id))?;
        let closure_name = format_ident!("on_{}", rust_method_name(&variant_name)?);
        let constructor_name = format_ident!("new_{}", rust_method_name(&variant_name)?);

        let body_id = get_edge(graph, variant_id, BODY_FIELD)
            .and_then(get_uuid)
            .ok_or_else(|| format!("Variant {} has no body", variant_id))?;

        if get_isa(graph, body_id) != Some(RECORD_T) {
            return Err(format!("Variant {} body is not a record", variant_id));
        }

        let variant_id = id_expr(variant_id);

        let record_field_ids = get_record_field_ids(graph, body_id)?;
        let (field_types, constructor_field_types, field_names, field_ids, conversions, field_setters) = record_field_ids.iter()
            .enumerate()
            .map(|(i, field_id)| {
                let field_name = format_ident!("f{}", i);
                let raw_id = format_ident!("raw_{}", i);

                let resolved = match get_edge(graph, field_id, TYPE_FIELD).and_then(get_uuid) {
                    Some(tid) => resolve_type(graph, tid, &full_subs)?,
                    None => return Err(format!("Field {} has no type", field_id)),
                };

                let field_id = id_expr(field_id);
                let (rust_type, converter) = resolved_type_to_rust(&resolved);
                let conversion = converter(quote! { #raw_id });

                let (constructor_field_type, field_setter) = match &resolved {
                    ResolvedType::String => (
                        quote! { impl Into<std::string::String> },
                        quote! { gid.set(id.clone(), #field_id, crate::graph::Id::String(#field_name.into())); }
                    ),
                    ResolvedType::Number => (
                        quote! { f64 },
                        quote! { gid.set(id.clone(), #field_id, crate::graph::Id::Number(ordered_float::OrderedFloat(#field_name))); }
                    ),
                    ResolvedType::Record { rust_name } => {
                        let wrapper = format_ident!("{}", rust_name);
                        (
                            quote! { &#wrapper },
                            quote! { gid.set(id.clone(), #field_id, #field_name.id().clone()); }
                        )
                    },
                    ResolvedType::Generic { rust_name, args } => {
                        let wrapper = format_ident!("{}", rust_name);
                        let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
                        (
                            quote! { &#wrapper<#(#arg_types),*> },
                            quote! { gid.set(id.clone(), #field_id, #field_name.id().clone()); }
                        )
                    },
                    ResolvedType::TypeParam { .. } => (
                        quote! { &crate::graph::Id },
                        quote! { gid.set(id.clone(), #field_id, #field_name.clone()); }
                    ),
                };

                Ok((rust_type, constructor_field_type, field_name, field_id, conversion, field_setter))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .fold(
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                |(mut types, mut ctor_types, mut names, mut ids, mut convs, mut setters), (t, ct, n, u, c, s)| {
                    types.push(t); ctor_types.push(ct); names.push(n); ids.push(u); convs.push(c); setters.push(s);
                    (types, ctor_types, names, ids, convs, setters)
                }
            );

        Ok(VariantInfo { closure_name, constructor_name, variant_id, field_types, constructor_field_types, field_names, field_ids, conversions, field_setters })
    }).collect::<Result<_>>()?;

    let self_id = if type_params.is_empty() {
        quote! { self.0 }
    } else {
        quote! { self.id }
    };

    let closure_params: Vec<_> = variants.iter().map(|v| {
        let closure_name = &v.closure_name;
        if v.field_types.is_empty() {
            quote! { #closure_name: impl FnOnce() -> __R }
        } else {
            let field_types = &v.field_types;
            quote! { #closure_name: impl FnOnce(#(#field_types),*) -> __R }
        }
    }).collect();

    let match_arms: Vec<_> = variants.iter().map(|v| {
        let closure_name = &v.closure_name;
        let variant_id = &v.variant_id;
        if v.field_types.is_empty() {
            quote! {
                id if id == &#variant_id => Some(#closure_name())
            }
        } else {
            let field_bindings: Vec<_> = v.field_names.iter().enumerate().map(|(i, field_name)| {
                let raw_id = format_ident!("raw_{}", i);
                let field_id = &v.field_ids[i];
                let conversion = &v.conversions[i];
                quote! {
                    let #raw_id = gid.get(&#self_id, &#field_id)?;
                    let #field_name = #conversion
                }
            }).collect();
            let field_names = &v.field_names;
            quote! {
                id if id == &#variant_id => {
                    #(#field_bindings;)*
                    Some(#closure_name(#(#field_names),*))
                }
            }
        }
    }).collect();

    if type_params.is_empty() {
        let variant_constructors: Vec<_> = variants.iter().map(|v| {
            let constructor_name = &v.constructor_name;
            let variant_id = &v.variant_id;
            let constructor_field_types = &v.constructor_field_types;
            let field_names = &v.field_names;
            let field_setters = &v.field_setters;
            if constructor_field_types.is_empty() {
                quote! {
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid) -> Self {
                        let id = crate::graph::Id::new_uuid();
                        gid.set(id.clone(), #isa_uuid, #variant_id);
                        Self(id)
                    }
                }
            } else {
                quote! {
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid, #(#field_names: #constructor_field_types),*) -> Self {
                        let id = crate::graph::Id::new_uuid();
                        gid.set(id.clone(), #isa_uuid, #variant_id);
                        #(#field_setters)*
                        Self(id)
                    }
                }
            }
        }).collect();

        let variant_uuids: Vec<_> = variants.iter().map(|v| &v.variant_id).collect();

        Ok(quote! {
            #[derive(Clone, Debug)]
            pub struct #struct_name(pub crate::graph::Id);

            impl #struct_name {
                pub const TYPE_ID: crate::graph::Id = #type_uuid;

                pub fn wrap(id: crate::graph::Id) -> Self {
                    Self(id)
                }

                pub fn try_wrap(gid: &impl crate::graph::Gid, id: &crate::graph::Id) -> Option<Self> {
                    let isa = gid.get(id, &#isa_uuid)?;
                    if #(isa == &#variant_uuids)||* {
                        Some(Self(id.clone()))
                    } else {
                        None
                    }
                }

                pub fn id(&self) -> &crate::graph::Id {
                    &self.0
                }

                #(#variant_constructors)*

                #[allow(clippy::too_many_arguments)]
                pub fn match_<__R>(
                    &self,
                    gid: &impl crate::graph::Gid,
                    #(#closure_params),*
                ) -> Option<__R> {
                    let isa = gid.get(&self.0, &#isa_uuid)?;
                    match isa {
                        #(#match_arms,)*
                        _ => None,
                    }
                }
            }
        })
    } else {
        let generic_params: Vec<_> = type_params.iter().map(|p| &p.rust_name).collect();
        let converter_fields: Vec<_> = type_params.iter().map(|p| {
            let name = &p.converter_name;
            let ty = &p.rust_name;
            quote! { #name: std::rc::Rc<dyn Fn(&crate::graph::Id) -> Option<#ty>> }
        }).collect();
        let converter_field_names: Vec<_> = type_params.iter().map(|p| &p.converter_name).collect();

        let variant_constructors: Vec<_> = variants.iter().map(|v| {
            let constructor_name = &v.constructor_name;
            let variant_id = &v.variant_id;
            let constructor_field_types = &v.constructor_field_types;
            let field_names = &v.field_names;
            let field_setters = &v.field_setters;
            if constructor_field_types.is_empty() {
                quote! {
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid, #(#converter_fields),*) -> Self {
                        let id = crate::graph::Id::new_uuid();
                        gid.set(id.clone(), #isa_uuid, #variant_id);
                        Self { id, #(#converter_field_names,)* }
                    }
                }
            } else {
                quote! {
                    #[allow(clippy::too_many_arguments)]
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid, #(#field_names: #constructor_field_types,)* #(#converter_fields),*) -> Self {
                        let id = crate::graph::Id::new_uuid();
                        gid.set(id.clone(), #isa_uuid, #variant_id);
                        #(#field_setters)*
                        Self { id, #(#converter_field_names,)* }
                    }
                }
            }
        }).collect();

        Ok(quote! {
            pub struct #struct_name<#(#generic_params),*> {
                pub id: crate::graph::Id,
                #(pub #converter_fields,)*
            }

            impl<#(#generic_params),*> Clone for #struct_name<#(#generic_params),*> {
                fn clone(&self) -> Self {
                    Self {
                        id: self.id.clone(),
                        #(#converter_field_names: self.#converter_field_names.clone(),)*
                    }
                }
            }

            impl<#(#generic_params),*> std::fmt::Debug for #struct_name<#(#generic_params),*> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(stringify!(#struct_name))
                        .field("id", &self.id)
                        .finish()
                }
            }

            impl<#(#generic_params),*> #struct_name<#(#generic_params),*> {
                pub const TYPE_ID: crate::graph::Id = #type_uuid;

                pub fn wrap(id: crate::graph::Id, #(#converter_fields),*) -> Self {
                    Self { id, #(#converter_field_names,)* }
                }

                pub fn id(&self) -> &crate::graph::Id {
                    &self.id
                }

                #(#variant_constructors)*

                #[allow(clippy::too_many_arguments)]
                pub fn match_<__R>(
                    &self,
                    gid: &impl crate::graph::Gid,
                    #(#closure_params),*
                ) -> Option<__R> {
                    let isa = gid.get(&self.id, &#isa_uuid)?;
                    match isa {
                        #(#match_arms,)*
                        _ => None,
                    }
                }
            }
        })
    }
}

fn generate_wrapper(graph: &Graph, type_id: &str, body_id: &str, type_name: &str, type_params: &[TypeParam], subs: &Substitutions) -> Result<TokenStream2> {
    let full_subs: Substitutions = subs.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(type_params.iter().map(|tp| (tp.id.clone(), ResolvedType::TypeParam {
            rust_name: tp.rust_name.to_string(),
            converter_name: tp.converter_name.to_string(),
        })))
        .collect();

    let field_ids = get_record_field_ids(graph, body_id)?;
    let struct_name = format_ident!("{}", rust_type_name(type_name)?);
    let type_uuid = id_expr(type_id);
    let isa_uuid = id_expr(ISA_FIELD);

    let self_id = if type_params.is_empty() {
        quote! { self.0 }
    } else {
        quote! { self.id }
    };

    let field_methods: Vec<TokenStream2> = field_ids
        .iter()
        .map(|field_id| generate_accessor(graph, field_id, &full_subs, &self_id))
        .collect::<Result<_>>()?;

    let field_setters: Vec<TokenStream2> = field_ids
        .iter()
        .map(|field_id| generate_setter(graph, field_id, &full_subs, &self_id))
        .collect::<Result<_>>()?;

    if type_params.is_empty() {
        Ok(quote! {
            #[derive(Clone, Debug)]
            pub struct #struct_name(pub crate::graph::Id);

            impl #struct_name {
                pub const TYPE_ID: crate::graph::Id = #type_uuid;

                pub fn new(gid: &mut crate::graph::MutGid) -> Self {
                    let id = crate::graph::Id::new_uuid();
                    gid.set(id.clone(), #isa_uuid, #type_uuid);
                    Self(id)
                }

                pub fn wrap(id: crate::graph::Id) -> Self {
                    Self(id)
                }

                pub fn try_wrap(gid: &impl crate::graph::Gid, id: &crate::graph::Id) -> Option<Self> {
                    if gid.get(id, &#isa_uuid) == Some(&#type_uuid) {
                        Some(Self(id.clone()))
                    } else {
                        None
                    }
                }

                pub fn id(&self) -> &crate::graph::Id {
                    &self.0
                }

                #(#field_methods)*

                #(#field_setters)*
            }
        })
    } else {
        let generic_params: Vec<_> = type_params.iter().map(|p| &p.rust_name).collect();
        let converter_fields: Vec<_> = type_params.iter().map(|p| {
            let name = &p.converter_name;
            let ty = &p.rust_name;
            quote! { #name: std::rc::Rc<dyn Fn(&crate::graph::Id) -> Option<#ty>> }
        }).collect();
        let converter_field_names: Vec<_> = type_params.iter().map(|p| &p.converter_name).collect();

        Ok(quote! {
            pub struct #struct_name<#(#generic_params),*> {
                pub id: crate::graph::Id,
                #(pub #converter_fields,)*
            }

            impl<#(#generic_params),*> Clone for #struct_name<#(#generic_params),*> {
                fn clone(&self) -> Self {
                    Self {
                        id: self.id.clone(),
                        #(#converter_field_names: self.#converter_field_names.clone(),)*
                    }
                }
            }

            impl<#(#generic_params),*> std::fmt::Debug for #struct_name<#(#generic_params),*> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(stringify!(#struct_name))
                        .field("id", &self.id)
                        .finish()
                }
            }

            impl<#(#generic_params),*> #struct_name<#(#generic_params),*> {
                pub const TYPE_ID: crate::graph::Id = #type_uuid;

                pub fn new(gid: &mut crate::graph::MutGid, #(#converter_fields),*) -> Self {
                    let id = crate::graph::Id::new_uuid();
                    gid.set(id.clone(), #isa_uuid, #type_uuid);
                    Self { id, #(#converter_field_names,)* }
                }

                pub fn wrap(id: crate::graph::Id, #(#converter_fields),*) -> Self {
                    Self { id, #(#converter_field_names,)* }
                }

                pub fn id(&self) -> &crate::graph::Id {
                    &self.id
                }

                #(#field_methods)*

                #(#field_setters)*
            }
        })
    }
}

fn load_semantics(input: TokenStream) -> Result<(String, Graph, Vec<Value>)> {
    let relative_path = syn::parse::<syn::LitStr>(input)
        .map_err(|e| format!("expected path string: {}", e))?
        .value();
    let full_path = format!("{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "CARGO_MANIFEST_DIR not set")?,
        relative_path);

    let json: Value = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("failed to read {}: {}", full_path, e))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("failed to parse JSON: {}", e)))?;

    Ok((
        relative_path,
        serde_json::from_value(json.get("graph").ok_or("missing graph field")?.clone())
            .map_err(|e| format!("failed to parse graph: {}", e))?,
        serde_json::from_value(json.get("roots").ok_or("missing roots field")?.clone())
            .map_err(|e| format!("failed to parse roots: {}", e))?,
    ))
}

fn generate_semantics_impl(input: TokenStream) -> Result<TokenStream2> {
    let (path, graph, roots) = load_semantics(input)?;
    let empty_subs = Substitutions::new();

    let wrappers: Vec<TokenStream2> = roots.iter()
        .filter_map(|root| {
            let id = get_uuid(root)?;
            let isa = get_isa(&graph, id)?;
            let name = get_name(&graph, id)?;
            if name == "String" || name == "Number" {
                return None;
            }
            match isa {
                TYPE_T => {
                    let body_id = get_edge(&graph, id, BODY_FIELD).and_then(get_uuid)?;
                    match get_isa(&graph, body_id) {
                        Some(RECORD_T) => Some(generate_wrapper(&graph, id, body_id, &name, &[], &empty_subs)),
                        Some(SUM_T) => Some(generate_sum_wrapper(&graph, id, body_id, &name, &[], &empty_subs)),
                        Some(FORALL_T) => {
                            let type_params = get_type_params(&graph, body_id).ok()?;
                            let forall_body_id = get_edge(&graph, body_id, BODY_FIELD).and_then(get_uuid)?;
                            match get_isa(&graph, forall_body_id) {
                                Some(RECORD_T) => Some(generate_wrapper(&graph, id, forall_body_id, &name, &type_params, &empty_subs)),
                                Some(SUM_T) => Some(generate_sum_wrapper(&graph, id, forall_body_id, &name, &type_params, &empty_subs)),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        })
        .collect::<Result<_>>()?;

    let field_constants: Vec<TokenStream2> = graph.keys()
        .filter(|id| get_isa(&graph, id) == Some(FIELD_T))
        .filter_map(|id| get_name(&graph, id).map(|name| (id, name)))
        .map(|(id, name)| {
            let const_name = format_ident!("{}", rust_const_name(&name)?);
            let id = id_expr(id);
            Ok(quote! { pub const #const_name: crate::graph::Id = #id; })
        })
        .collect::<Result<_>>()?;

    let type_constants: Vec<TokenStream2> = graph.keys()
        .filter(|id| get_isa(&graph, id) == Some(TYPE_T))
        .filter_map(|id| get_name(&graph, id).map(|name| (id, name)))
        .map(|(id, name)| {
            let const_name = format_ident!("{}_TYPE", rust_const_name(&name)?);
            let id = id_expr(id);
            Ok(quote! { pub const #const_name: crate::graph::Id = #id; })
        })
        .collect::<Result<_>>()?;

    let include_path = format!("/{}", path);
    Ok(quote! {
        const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), #include_path));
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
