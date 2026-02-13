use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use progred_graph::{Id, Gid, MutGid};

// Core semantic field IDs (from saved semantics.progred)
const NAME: Id = Id::Uuid(Uuid::from_bytes([0x21, 0xab, 0x34, 0x39, 0xef, 0x04, 0x47, 0x83, 0x90, 0x22, 0x3d, 0x04, 0x00, 0xde, 0x4b, 0xbb]));
const ISA: Id = Id::Uuid(Uuid::from_bytes([0x1d, 0x75, 0xe2, 0x3d, 0x71, 0x47, 0x46, 0x6a, 0xa3, 0x3d, 0x38, 0x42, 0x86, 0xfa, 0x51, 0x8d]));
const BODY: Id = Id::Uuid(Uuid::from_bytes([0x26, 0x95, 0x6a, 0x6c, 0xe4, 0x25, 0x4b, 0x1d, 0xbc, 0xd7, 0xfc, 0x2d, 0xba, 0x14, 0xb2, 0x77]));
const BASE: Id = Id::Uuid(Uuid::from_bytes([0x4c, 0x81, 0xa5, 0x76, 0x78, 0x58, 0x46, 0xed, 0x82, 0x46, 0x12, 0x15, 0x8e, 0x0c, 0x4a, 0x7a]));
const ARGS: Id = Id::Uuid(Uuid::from_bytes([0x1d, 0x43, 0x86, 0x8e, 0x58, 0x73, 0x45, 0xdc, 0xa3, 0xb1, 0x5f, 0x8c, 0xef, 0x0b, 0x41, 0x0e]));
const FIELDS: Id = Id::Uuid(Uuid::from_bytes([0xf3, 0x55, 0xea, 0xd0, 0x6b, 0xfb, 0x45, 0x0e, 0xa7, 0x29, 0x17, 0x35, 0xc3, 0x95, 0x0a, 0x0e]));
const TYPE_FIELD: Id = Id::Uuid(Uuid::from_bytes([0x7f, 0x32, 0x8a, 0xc9, 0xde, 0x07, 0x45, 0xa5, 0x86, 0xb3, 0x02, 0x1b, 0x27, 0x11, 0x74, 0x7a]));
const HEAD: Id = Id::Uuid(Uuid::from_bytes([0x83, 0xf7, 0x24, 0xd9, 0x10, 0x08, 0x43, 0xe3, 0x80, 0x7c, 0xf6, 0x6a, 0xdc, 0x7d, 0x77, 0x4f]));
const TAIL: Id = Id::Uuid(Uuid::from_bytes([0xcf, 0xfc, 0x46, 0xb8, 0x23, 0x88, 0x41, 0x97, 0xad, 0x87, 0x7e, 0x7e, 0xf4, 0x46, 0xac, 0xcb]));

// Type construct IDs
const TYPE_T: Id = Id::Uuid(Uuid::from_bytes([0xb7, 0x18, 0x58, 0x15, 0xd5, 0x53, 0x4f, 0xbf, 0x90, 0x25, 0xd8, 0x86, 0x43, 0xa7, 0xba, 0x6a]));
const APPLY_T: Id = Id::Uuid(Uuid::from_bytes([0xd6, 0xad, 0x02, 0x73, 0x88, 0x6d, 0x4e, 0x83, 0x9e, 0xd7, 0xe4, 0xfe, 0x5f, 0xe2, 0xf4, 0xe8]));
const RECORD_T: Id = Id::Uuid(Uuid::from_bytes([0x4a, 0xe8, 0xa1, 0xde, 0xf7, 0xf5, 0x47, 0x33, 0xb6, 0xca, 0x0e, 0x01, 0x86, 0x26, 0x35, 0xe6]));
const FIELD_T: Id = Id::Uuid(Uuid::from_bytes([0xc9, 0xbd, 0xef, 0x1a, 0xc9, 0xb0, 0x49, 0x27, 0xa8, 0x81, 0x5c, 0xf7, 0xfa, 0x54, 0xbb, 0x9b]));
const FORALL_T: Id = Id::Uuid(Uuid::from_bytes([0x2e, 0x37, 0x84, 0xa7, 0x05, 0x42, 0x44, 0xbe, 0x90, 0xde, 0x9c, 0xa8, 0xf8, 0xa6, 0x06, 0xa4]));
const SUM_T: Id = Id::Uuid(Uuid::from_bytes([0x68, 0x47, 0x44, 0x9f, 0xa1, 0x22, 0x40, 0xf5, 0x88, 0x80, 0x16, 0x92, 0xd9, 0x68, 0xd1, 0x27]));
const PARAMS: Id = Id::Uuid(Uuid::from_bytes([0x10, 0x65, 0x0b, 0x24, 0x48, 0x76, 0x47, 0x26, 0xbd, 0x57, 0xb0, 0x6c, 0xe8, 0xfc, 0xf8, 0x74]));
const VARIANTS: Id = Id::Uuid(Uuid::from_bytes([0xde, 0x15, 0xc5, 0xce, 0xf6, 0x5c, 0x4e, 0x32, 0xa1, 0x5e, 0x94, 0xf3, 0x7d, 0xbb, 0x75, 0x41]));

// Primitive type IDs
const STRING_T: Id = Id::Uuid(Uuid::from_bytes([0x35, 0x5e, 0x01, 0xb2, 0x4c, 0xbd, 0x41, 0x3a, 0xa8, 0x9d, 0xe0, 0x5f, 0xba, 0x7c, 0x57, 0x7d]));
const NUMBER_T: Id = Id::Uuid(Uuid::from_bytes([0xd4, 0x32, 0x50, 0x76, 0xd3, 0x3e, 0x4f, 0xc8, 0xb3, 0xf6, 0x5b, 0xa8, 0x53, 0x85, 0xa3, 0x52]));

// List bootstrap IDs
const CONS_T: Id = Id::Uuid(Uuid::from_bytes([0x26, 0x4e, 0x80, 0xb0, 0xa8, 0xe1, 0x4f, 0xc2, 0xa3, 0xbd, 0x6c, 0x10, 0xff, 0x17, 0x72, 0x61]));
const EMPTY_T: Id = Id::Uuid(Uuid::from_bytes([0x1d, 0xff, 0x7f, 0x8a, 0x1d, 0x83, 0x41, 0xfd, 0xaf, 0x2d, 0x08, 0x7b, 0x75, 0x3e, 0xd0, 0x08]));


type Result<T> = std::result::Result<T, String>;
type Substitutions = HashMap<Id, ResolvedType>;

fn get_name(gid: &impl Gid, id: &Id) -> Option<String> {
    match gid.get(id, &NAME)? {
        Id::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn get_isa<'a>(gid: &'a impl Gid, id: &Id) -> Option<&'a Id> {
    gid.get(id, &ISA)
}

fn flatten_list(gid: &impl Gid, list_id: &Id) -> Vec<Id> {
    let mut result = Vec::new();
    let mut current = list_id.clone();
    let mut seen = std::collections::HashSet::new();

    while seen.insert(current.clone()) {
        let isa = get_isa(gid, &current);
        if isa == Some(&CONS_T) {
            if let Some(head_id) = gid.get(&current, &HEAD) {
                result.push(head_id.clone());
            }
            match gid.get(&current, &TAIL) {
                Some(tail_id) => current = tail_id.clone(),
                None => break,
            }
        } else if isa == Some(&EMPTY_T) {
            break;
        } else {
            break;
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

fn get_forall_params(gid: &impl Gid, type_id: &Id) -> Option<Vec<Id>> {
    let body_id = gid.get(type_id, &BODY)?;
    if get_isa(gid, body_id) == Some(&FORALL_T) {
        let params_list_id = gid.get(body_id, &PARAMS)?;
        Some(flatten_list(gid, params_list_id))
    } else {
        None
    }
}

fn resolve_type(gid: &impl Gid, type_id: &Id, subs: &Substitutions) -> Result<ResolvedType> {
    if let Some(resolved) = subs.get(type_id) {
        return Ok(resolved.clone());
    }

    if *type_id == STRING_T {
        return Ok(ResolvedType::String);
    }
    if *type_id == NUMBER_T {
        return Ok(ResolvedType::Number);
    }

    let isa = get_isa(gid, type_id);
    if isa == Some(&RECORD_T) {
        return Err(format!("Bare record {} not wrapped in named type", type_id));
    }
    if isa == Some(&SUM_T) {
        return Err(format!("Bare sum {} not wrapped in named type", type_id));
    }
    if isa == Some(&TYPE_T) {
        let body_id = gid.get(type_id, &BODY)
            .ok_or_else(|| format!("Type {} has no body", type_id))?;
        let body_isa = get_isa(gid, body_id);
        if body_isa == Some(&RECORD_T) || body_isa == Some(&SUM_T) {
            return get_name(gid, type_id)
                .ok_or_else(|| format!("Type {} has no name", type_id))
                .and_then(|name| rust_type_name(&name).map(|rust_name| ResolvedType::Record { rust_name }));
        }
        return resolve_type(gid, body_id, subs);
    }
    if isa == Some(&APPLY_T) {
        let base_id = gid.get(type_id, &BASE)
            .ok_or_else(|| format!("Apply {} has no base", type_id))?;
        let args_list_id = gid.get(type_id, &ARGS)
            .ok_or_else(|| format!("Apply {} has no args", type_id))?;
        let arg_ids = flatten_list(gid, args_list_id);

        if get_forall_params(gid, base_id).is_some() {
            let rust_name = get_name(gid, base_id)
                .ok_or_else(|| format!("Generic type {} has no name", base_id))
                .and_then(|name| rust_type_name(&name))?;
            let args = arg_ids
                .iter()
                .map(|a| resolve_type(gid, a, subs))
                .collect::<Result<_>>()?;
            return Ok(ResolvedType::Generic { rust_name, args });
        }
        return Err(format!("Apply {} to non-generic type {}", type_id, base_id));
    }
    Err(format!("Unknown type structure: {}", type_id))
}

fn get_record_field_ids(gid: &impl Gid, record_id: &Id) -> Result<Vec<Id>> {
    let fields_list_id = gid.get(record_id, &FIELDS)
        .ok_or_else(|| format!("Record {} has no fields list", record_id))?;
    Ok(flatten_list(gid, fields_list_id))
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

fn generate_setter(gid: &impl Gid, field_id: &Id, subs: &Substitutions, self_id: &TokenStream2) -> Result<TokenStream2> {
    let field_name = get_name(gid, field_id)
        .ok_or_else(|| format!("Field {} has no name", field_id))?;
    let field_type = match gid.get(field_id, &TYPE_FIELD) {
        Some(t) => resolve_type(gid, t, subs)?,
        None => return Err(format!("Field {} has no type", field_id)),
    };
    let method_name = format_ident!("set_{}", rust_method_name(&field_name)?);
    let field_id = id_expr(field_id);
    let entity = quote! { match &#self_id { crate::graph::Id::Uuid(u) => *u, _ => unreachable!() } };

    Ok(match field_type {
        ResolvedType::String => quote! {
            pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: impl Into<std::string::String>) {
                gid.set(#entity, #field_id, crate::graph::Id::String(value.into()));
            }
        },
        ResolvedType::Number => quote! {
            pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: f64) {
                gid.set(#entity, #field_id, crate::graph::Id::Number(ordered_float::OrderedFloat(value)));
            }
        },
        ResolvedType::Record { rust_name } => {
            let wrapper_name = format_ident!("{}", rust_name);
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &#wrapper_name) {
                    gid.set(#entity, #field_id, value.id().clone());
                }
            }
        },
        ResolvedType::Generic { rust_name, args } => {
            let wrapper = format_ident!("{}", rust_name);
            let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &#wrapper<#(#arg_types),*>) {
                    gid.set(#entity, #field_id, value.id().clone());
                }
            }
        },
        ResolvedType::TypeParam { .. } => {
            quote! {
                pub fn #method_name(&self, gid: &mut crate::graph::MutGid, value: &crate::graph::Id) {
                    gid.set(#entity, #field_id, value.clone());
                }
            }
        },
    })
}

fn generate_accessor(gid: &impl Gid, field_id: &Id, subs: &Substitutions, self_id: &TokenStream2) -> Result<TokenStream2> {
    let field_name = get_name(gid, field_id)
        .ok_or_else(|| format!("Field {} has no name", field_id))?;
    let field_type = match gid.get(field_id, &TYPE_FIELD) {
        Some(t) => resolve_type(gid, t, subs)?,
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

fn get_sum_variant_ids(gid: &impl Gid, sum_id: &Id) -> Result<Vec<Id>> {
    let variants_list_id = gid.get(sum_id, &VARIANTS)
        .ok_or_else(|| format!("Sum {} has no variants list", sum_id))?;
    Ok(flatten_list(gid, variants_list_id))
}

struct TypeParam {
    id: Id,
    rust_name: syn::Ident,
    converter_name: syn::Ident,
}

fn get_type_params(gid: &impl Gid, forall_id: &Id) -> Result<Vec<TypeParam>> {
    let params_list_id = gid.get(forall_id, &PARAMS)
        .ok_or_else(|| format!("Forall {} has no params", forall_id))?;

    flatten_list(gid, params_list_id)
        .into_iter()
        .map(|param_id| {
            let name = get_name(gid, &param_id)
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

fn generate_sum_wrapper(gid: &impl Gid, type_id: &Id, sum_id: &Id, type_name: &str, type_params: &[TypeParam], subs: &Substitutions) -> Result<TokenStream2> {
    let full_subs: Substitutions = subs.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(type_params.iter().map(|tp| (tp.id.clone(), ResolvedType::TypeParam {
            rust_name: tp.rust_name.to_string(),
            converter_name: tp.converter_name.to_string(),
        })))
        .collect();

    let variant_ids = get_sum_variant_ids(gid, sum_id)?;
    let struct_name = format_ident!("{}", rust_type_name(type_name)?);
    let type_uuid = id_expr(type_id);
    let isa_uuid = id_expr(&ISA);

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
        let variant_name = get_name(gid, variant_id)
            .ok_or_else(|| format!("Variant {} has no name", variant_id))?;
        let closure_name = format_ident!("on_{}", rust_method_name(&variant_name)?);
        let constructor_name = format_ident!("new_{}", rust_method_name(&variant_name)?);

        let body_id = gid.get(variant_id, &BODY)
            .ok_or_else(|| format!("Variant {} has no body", variant_id))?;

        if get_isa(gid, body_id) != Some(&RECORD_T) {
            return Err(format!("Variant {} body is not a record", variant_id));
        }

        let variant_id_tokens = id_expr(variant_id);

        let record_field_ids = get_record_field_ids(gid, body_id)?;
        let (field_types, constructor_field_types, field_names, field_ids, conversions, field_setters) = record_field_ids.iter()
            .enumerate()
            .map(|(i, field_id)| {
                let field_name = format_ident!("f{}", i);
                let raw_id = format_ident!("raw_{}", i);

                let resolved = match gid.get(field_id, &TYPE_FIELD) {
                    Some(tid) => resolve_type(gid, tid, &full_subs)?,
                    None => return Err(format!("Field {} has no type", field_id)),
                };

                let field_id_tokens = id_expr(field_id);
                let (rust_type, converter) = resolved_type_to_rust(&resolved);
                let conversion = converter(quote! { #raw_id });

                let (constructor_field_type, field_setter) = match &resolved {
                    ResolvedType::String => (
                        quote! { impl Into<std::string::String> },
                        quote! { gid.set(uuid, #field_id_tokens, crate::graph::Id::String(#field_name.into())); }
                    ),
                    ResolvedType::Number => (
                        quote! { f64 },
                        quote! { gid.set(uuid, #field_id_tokens, crate::graph::Id::Number(ordered_float::OrderedFloat(#field_name))); }
                    ),
                    ResolvedType::Record { rust_name } => {
                        let wrapper = format_ident!("{}", rust_name);
                        (
                            quote! { &#wrapper },
                            quote! { gid.set(uuid, #field_id_tokens, #field_name.id().clone()); }
                        )
                    },
                    ResolvedType::Generic { rust_name, args } => {
                        let wrapper = format_ident!("{}", rust_name);
                        let arg_types: Vec<_> = args.iter().map(|a| resolved_type_to_rust(a).0).collect();
                        (
                            quote! { &#wrapper<#(#arg_types),*> },
                            quote! { gid.set(uuid, #field_id_tokens, #field_name.id().clone()); }
                        )
                    },
                    ResolvedType::TypeParam { .. } => (
                        quote! { &crate::graph::Id },
                        quote! { gid.set(uuid, #field_id_tokens, #field_name.clone()); }
                    ),
                };

                Ok((rust_type, constructor_field_type, field_name, field_id_tokens, conversion, field_setter))
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

        Ok(VariantInfo { closure_name, constructor_name, variant_id: variant_id_tokens, field_types, constructor_field_types, field_names, field_ids, conversions, field_setters })
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
                        let uuid = uuid::Uuid::new_v4();
                        let id = crate::graph::Id::Uuid(uuid);
                        gid.set(uuid, #isa_uuid, #variant_id);
                        Self(id)
                    }
                }
            } else {
                quote! {
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid, #(#field_names: #constructor_field_types),*) -> Self {
                        let uuid = uuid::Uuid::new_v4();
                        let id = crate::graph::Id::Uuid(uuid);
                        gid.set(uuid, #isa_uuid, #variant_id);
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
                        let uuid = uuid::Uuid::new_v4();
                        let id = crate::graph::Id::Uuid(uuid);
                        gid.set(uuid, #isa_uuid, #variant_id);
                        Self { id, #(#converter_field_names,)* }
                    }
                }
            } else {
                quote! {
                    #[allow(clippy::too_many_arguments)]
                    pub fn #constructor_name(gid: &mut crate::graph::MutGid, #(#field_names: #constructor_field_types,)* #(#converter_fields),*) -> Self {
                        let uuid = uuid::Uuid::new_v4();
                        let id = crate::graph::Id::Uuid(uuid);
                        gid.set(uuid, #isa_uuid, #variant_id);
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

fn generate_wrapper(gid: &impl Gid, type_id: &Id, body_id: &Id, type_name: &str, type_params: &[TypeParam], subs: &Substitutions) -> Result<TokenStream2> {
    let full_subs: Substitutions = subs.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(type_params.iter().map(|tp| (tp.id.clone(), ResolvedType::TypeParam {
            rust_name: tp.rust_name.to_string(),
            converter_name: tp.converter_name.to_string(),
        })))
        .collect();

    let field_ids = get_record_field_ids(gid, body_id)?;
    let struct_name = format_ident!("{}", rust_type_name(type_name)?);
    let type_uuid = id_expr(type_id);
    let isa_uuid = id_expr(&ISA);

    let self_id = if type_params.is_empty() {
        quote! { self.0 }
    } else {
        quote! { self.id }
    };

    let field_methods: Vec<TokenStream2> = field_ids
        .iter()
        .map(|field_id| generate_accessor(gid, field_id, &full_subs, &self_id))
        .collect::<Result<_>>()?;

    let field_setters: Vec<TokenStream2> = field_ids
        .iter()
        .map(|field_id| generate_setter(gid, field_id, &full_subs, &self_id))
        .collect::<Result<_>>()?;

    if type_params.is_empty() {
        Ok(quote! {
            #[derive(Clone, Debug)]
            pub struct #struct_name(pub crate::graph::Id);

            impl #struct_name {
                pub const TYPE_ID: crate::graph::Id = #type_uuid;

                pub fn new(gid: &mut crate::graph::MutGid) -> Self {
                    let uuid = uuid::Uuid::new_v4();
                    let id = crate::graph::Id::Uuid(uuid);
                    gid.set(uuid, #isa_uuid, #type_uuid);
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
                    let uuid = uuid::Uuid::new_v4();
                    let id = crate::graph::Id::Uuid(uuid);
                    gid.set(uuid, #isa_uuid, #type_uuid);
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

fn load_semantics(input: TokenStream) -> Result<(String, MutGid, Vec<Id>)> {
    let relative_path = syn::parse::<syn::LitStr>(input)
        .map_err(|e| format!("expected path string: {}", e))?
        .value();
    let full_path = format!("{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "CARGO_MANIFEST_DIR not set")?,
        relative_path);

    let json: Value = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("failed to read {}: {}", full_path, e))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("failed to parse JSON: {}", e)))?;

    let graph_json: HashMap<String, HashMap<String, Value>> =
        serde_json::from_value(json.get("graph").ok_or("missing graph field")?.clone())
            .map_err(|e| format!("failed to parse graph: {}", e))?;
    let gid = MutGid::from_json(graph_json)
        .map_err(|e| format!("failed to load graph: {}", e))?;

    let roots: Vec<Id> = serde_json::from_value(json.get("roots").ok_or("missing roots field")?.clone())
        .map_err(|e| format!("failed to parse roots: {}", e))?;

    Ok((relative_path, gid, roots))
}

fn generate_semantics_impl(input: TokenStream) -> Result<TokenStream2> {
    let (path, gid, roots) = load_semantics(input)?;
    let empty_subs = Substitutions::new();

    let wrappers: Vec<TokenStream2> = roots.iter()
        .filter_map(|id| {
            let isa = get_isa(&gid, id)?;
            let name = get_name(&gid, id)?;
            if name == "String" || name == "Number" {
                return None;
            }
            if isa != &TYPE_T {
                return None;
            }
            let body_id = gid.get(id, &BODY)?;
            let body_isa = get_isa(&gid, body_id)?;
            if body_isa == &RECORD_T {
                Some(generate_wrapper(&gid, id, body_id, &name, &[], &empty_subs))
            } else if body_isa == &SUM_T {
                Some(generate_sum_wrapper(&gid, id, body_id, &name, &[], &empty_subs))
            } else if body_isa == &FORALL_T {
                let type_params = get_type_params(&gid, body_id).ok()?;
                let forall_body_id = gid.get(body_id, &BODY)?;
                let forall_body_isa = get_isa(&gid, forall_body_id)?;
                if forall_body_isa == &RECORD_T {
                    Some(generate_wrapper(&gid, id, forall_body_id, &name, &type_params, &empty_subs))
                } else if forall_body_isa == &SUM_T {
                    Some(generate_sum_wrapper(&gid, id, forall_body_id, &name, &type_params, &empty_subs))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Result<_>>()?;

    let field_constants: Vec<TokenStream2> = gid.entities()
        .filter_map(|uuid| {
            let id = Id::Uuid(*uuid);
            if get_isa(&gid, &id)? == &FIELD_T {
                let name = get_name(&gid, &id)?;
                Some((id, name))
            } else {
                None
            }
        })
        .map(|(id, name)| {
            let const_name = format_ident!("{}", rust_const_name(&name)?);
            let id_tokens = id_expr(&id);
            Ok(quote! { pub const #const_name: crate::graph::Id = #id_tokens; })
        })
        .collect::<Result<_>>()?;

    let type_constants: Vec<TokenStream2> = gid.entities()
        .filter_map(|uuid| {
            let id = Id::Uuid(*uuid);
            if get_isa(&gid, &id)? == &TYPE_T {
                let name = get_name(&gid, &id)?;
                Some((id, name))
            } else {
                None
            }
        })
        .map(|(id, name)| {
            let const_name = format_ident!("{}_TYPE", rust_const_name(&name)?);
            let id_tokens = id_expr(&id);
            Ok(quote! { pub const #const_name: crate::graph::Id = #id_tokens; })
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
