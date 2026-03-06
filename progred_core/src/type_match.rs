use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use crate::path::Path;
use std::collections::HashMap;

pub fn expected_type(gid: &impl Gid, path: &Path) -> Option<TypeExpression> {
    expected_type_env(gid, path).map(|(te, _)| te)
}

fn expected_type_env(gid: &impl Gid, path: &Path) -> Option<(TypeExpression, HashMap<Id, Id>)> {
    let (parent, label) = path.pop()?;
    let parent_env = expected_type_env(gid, &parent);
    let subs = parent_env.as_ref()
        .map(|(te, outer_subs)| substitutions(gid, te, outer_subs))
        .unwrap_or_default();
    let raw_type = gid.get(&label, &TYPE_)?;
    let resolved = subs.get(raw_type).unwrap_or(raw_type);
    Some((TypeExpression::wrap(resolved.clone()), subs))
}

fn substitutions(gid: &impl Gid, type_id: &Id, outer_subs: &HashMap<Id, Id>) -> HashMap<Id, Id> {
    Apply::try_wrap(gid, type_id)
        .and_then(|apply| {
            let base = apply.base(gid)?;
            let base_body = gid.get(&base, &BODY)?;
            let params_id = gid.get(base_body, &PARAMS)?;
            let args_id = gid.get(type_id, &ARGS)?;
            Some((params_id, args_id))
        })
        .map(|(params_id, args_id)| {
            ListIter::new(gid, Some(params_id))
                .zip(ListIter::new(gid, Some(args_id)))
                .map(|(param, arg)| (param.clone(), outer_subs.get(arg).cloned().unwrap_or_else(|| arg.clone())))
                .collect()
        })
        .unwrap_or_default()
}

pub fn type_matches(gid: &impl Gid, candidate: &Id, expected: &TypeExpression) -> Option<bool> {
    match candidate {
        Id::String(_) => Some(expected.id == STRING_TYPE || contains_atomic(gid, expected, &STRING_TYPE)),
        Id::Number(_) => Some(expected.id == NUMBER_TYPE || contains_atomic(gid, expected, &NUMBER_TYPE)),
        Id::Uuid(_) => gid.get(candidate, &ISA)
            .and_then(|isa| isa_matches(gid, isa, expected))
            .or(Some(false)),
    }
}

pub fn isa_matches_type(gid: &impl Gid, candidate_isa: &Id, expected: &TypeExpression) -> Option<bool> {
    isa_matches(gid, candidate_isa, expected)
}

pub fn type_contains_atomic(gid: &impl Gid, expected: &TypeExpression, atomic_type: &Id) -> bool {
    contains_atomic(gid, expected, atomic_type)
}

fn isa_matches(gid: &impl Gid, candidate_isa: &Id, expected: &Id) -> Option<bool> {
    if Type::try_wrap(gid, expected).is_some() {
        if candidate_isa == expected {
            Some(true)
        } else {
            gid.get(expected, &BODY)
                .and_then(|body| isa_matches(gid, candidate_isa, body))
                .or(Some(false))
        }
    } else if Sum::try_wrap(gid, expected).is_some() {
        Some(gid.get(expected, &VARIANTS)
            .map_or(false, |variants| ListIter::new(gid, Some(variants)).any(|v| v == candidate_isa)))
    } else if Apply::try_wrap(gid, expected).is_some() {
        gid.get(expected, &BASE)
            .and_then(|base| isa_matches(gid, candidate_isa, base))
            .or(Some(false))
    } else if Forall::try_wrap(gid, expected).is_some() {
        gid.get(expected, &BODY)
            .and_then(|body| isa_matches(gid, candidate_isa, body))
            .or(Some(false))
    } else if Record::try_wrap(gid, expected).is_some() {
        Some(candidate_isa == expected)
    } else {
        None
    }
}

fn contains_atomic(gid: &impl Gid, type_id: &Id, atomic_type: &Id) -> bool {
    if type_id == atomic_type {
        true
    } else if Type::try_wrap(gid, type_id).is_some() || Forall::try_wrap(gid, type_id).is_some() {
        gid.get(type_id, &BODY)
            .map_or(false, |body| contains_atomic(gid, body, atomic_type))
    } else if Sum::try_wrap(gid, type_id).is_some() {
        gid.get(type_id, &VARIANTS)
            .map_or(false, |variants| {
                ListIter::new(gid, Some(variants))
                    .any(|v| contains_atomic(gid, v, atomic_type))
            })
    } else if Apply::try_wrap(gid, type_id).is_some() {
        gid.get(type_id, &BASE)
            .map_or(false, |base| contains_atomic(gid, base, atomic_type))
    } else {
        false
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;

    #[test]
    fn expected_type_for_field() {
        let mut gid = MutGid::new();
        let field = Field::new(&mut gid);
        field.set_type_(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));
        let path = Path::orphan(Id::new_uuid()).child(field.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn string_matches_string_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(STRING_TYPE.clone());
        assert_eq!(type_matches(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn number_matches_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        assert_eq!(type_matches(&gid, &Id::Number(ordered_float::OrderedFloat(42.0)), &et), Some(true));
    }

    #[test]
    fn string_does_not_match_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        assert_eq!(type_matches(&gid, &Id::String("hello".into()), &et), Some(false));
    }

    #[test]
    fn uuid_with_matching_isa() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let record = Record::new(&mut gid);
        t.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), t.id().clone());
        let et = TypeExpression::wrap(t.id().clone());
        assert_eq!(type_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    fn te_conv() -> std::rc::Rc<dyn Fn(&Id) -> Option<TypeExpression>> {
        std::rc::Rc::new(|id| Some(TypeExpression::wrap(id.clone())))
    }

    fn make_generic_type(gid: &mut MutGid, params: &[&Id]) -> (Type, Forall) {
        let conv = te_conv();
        let empty = List::new_empty(gid, conv.clone());
        let params_list = params.iter().rev().fold(empty, |tail, param| {
            List::new_cons(gid, param, &tail, conv.clone())
        });
        let forall = Forall::new(gid);
        forall.set_params(gid, &params_list);
        let t = Type::new(gid);
        t.set_body(gid, &TypeExpression::wrap(forall.id().clone()));
        (t, forall)
    }

    fn make_apply(gid: &mut MutGid, base: &Type, args: &[&Id]) -> Apply {
        let conv = te_conv();
        let empty = List::new_empty(gid, conv.clone());
        let args_list = args.iter().rev().fold(empty, |tail, arg| {
            List::new_cons(gid, arg, &tail, conv.clone())
        });
        let apply = Apply::new(gid);
        apply.set_base(gid, base);
        apply.set_args(gid, &args_list);
        apply
    }

    fn make_field(gid: &mut MutGid, type_id: &Id) -> Field {
        let f = Field::new(gid);
        f.set_type_(gid, &TypeExpression::wrap(type_id.clone()));
        f
    }

    #[test]
    fn type_param_resolved_through_apply() {
        let mut gid = MutGid::new();
        let param_t = Type::new(&mut gid);
        let (box_type, _) = make_generic_type(&mut gid, &[param_t.id()]);
        let apply = make_apply(&mut gid, &box_type, &[&STRING_TYPE]);

        let outer = make_field(&mut gid, apply.id());
        let inner = make_field(&mut gid, param_t.id());

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id().clone())
            .child(inner.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn multiple_type_params() {
        let mut gid = MutGid::new();
        let param_a = Type::new(&mut gid);
        let param_b = Type::new(&mut gid);
        let (pair_type, _) = make_generic_type(&mut gid, &[param_a.id(), param_b.id()]);
        let apply = make_apply(&mut gid, &pair_type, &[&STRING_TYPE, &NUMBER_TYPE]);

        let pair_field = make_field(&mut gid, apply.id());
        let first = make_field(&mut gid, param_a.id());
        let second = make_field(&mut gid, param_b.id());

        let root = Id::new_uuid();
        let path_first = Path::orphan(root.clone())
            .child(pair_field.id().clone())
            .child(first.id().clone());
        assert_eq!(expected_type(&gid, &path_first).unwrap().id, STRING_TYPE);

        let path_second = Path::orphan(root)
            .child(pair_field.id().clone())
            .child(second.id().clone());
        assert_eq!(expected_type(&gid, &path_second).unwrap().id, NUMBER_TYPE);
    }

    #[test]
    fn recursive_type_through_self_apply() {
        let mut gid = MutGid::new();
        let param_t = Type::new(&mut gid);
        let (list_type, forall) = make_generic_type(&mut gid, &[param_t.id()]);

        let head_field = make_field(&mut gid, param_t.id());
        let tail_apply = make_apply(&mut gid, &list_type, &[param_t.id()]);
        let tail_field = make_field(&mut gid, tail_apply.id());
        forall.set_body(&mut gid, &TypeExpression::wrap(Id::new_uuid()));

        let concrete = make_apply(&mut gid, &list_type, &[&STRING_TYPE]);
        let list_field = make_field(&mut gid, concrete.id());

        let root = Id::new_uuid();
        let path_head = Path::orphan(root.clone())
            .child(list_field.id().clone())
            .child(head_field.id().clone());
        assert_eq!(expected_type(&gid, &path_head).unwrap().id, STRING_TYPE);

        let path_tail_head = Path::orphan(root)
            .child(list_field.id().clone())
            .child(tail_field.id().clone())
            .child(head_field.id().clone());
        assert_eq!(expected_type(&gid, &path_tail_head).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn deep_recursive_resolution() {
        let mut gid = MutGid::new();
        let param_t = Type::new(&mut gid);
        let (list_type, _) = make_generic_type(&mut gid, &[param_t.id()]);

        let head_field = make_field(&mut gid, param_t.id());
        let tail_apply = make_apply(&mut gid, &list_type, &[param_t.id()]);
        let tail_field = make_field(&mut gid, tail_apply.id());

        let concrete = make_apply(&mut gid, &list_type, &[&NUMBER_TYPE]);
        let list_field = make_field(&mut gid, concrete.id());

        let path = Path::orphan(Id::new_uuid())
            .child(list_field.id().clone())
            .child(tail_field.id().clone())
            .child(tail_field.id().clone())
            .child(tail_field.id().clone())
            .child(head_field.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, NUMBER_TYPE);
    }

    #[test]
    fn nested_type_param_resolution() {
        let mut gid = MutGid::new();

        let inner_param = Type::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[inner_param.id()]);

        let outer_param = Type::new(&mut gid);
        let (outer_type, outer_forall) = make_generic_type(&mut gid, &[outer_param.id()]);
        let inner_apply = make_apply(&mut gid, &inner_type, &[outer_param.id()]);
        let wrapper_field = make_field(&mut gid, inner_apply.id());
        let record = Record::new(&mut gid);
        let field_conv = std::rc::Rc::new(|id: &Id| Some(Field::wrap(id.clone())));
        let empty_fields = List::new_empty(&mut gid, field_conv.clone());
        let fields_list = List::new_cons(&mut gid, wrapper_field.id(), &empty_fields, field_conv);
        record.set_fields(&mut gid, &fields_list);
        outer_forall.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        let outer_apply = make_apply(&mut gid, &outer_type, &[&NUMBER_TYPE]);
        let container_field = make_field(&mut gid, outer_apply.id());
        let leaf_field = make_field(&mut gid, inner_param.id());

        let path = Path::orphan(Id::new_uuid())
            .child(container_field.id().clone())
            .child(wrapper_field.id().clone())
            .child(leaf_field.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, NUMBER_TYPE);
    }

    #[test]
    fn nested_recursive_types() {
        let mut gid = MutGid::new();

        let inner_param = Type::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[inner_param.id()]);
        let inner_head = make_field(&mut gid, inner_param.id());
        let inner_tail_apply = make_apply(&mut gid, &inner_type, &[inner_param.id()]);
        let inner_tail = make_field(&mut gid, inner_tail_apply.id());

        let outer_param = Type::new(&mut gid);
        let (outer_type, _) = make_generic_type(&mut gid, &[outer_param.id()]);
        let outer_value = make_field(&mut gid, outer_param.id());
        let inner_of_outer = make_apply(&mut gid, &inner_type, &[outer_param.id()]);
        let outer_list_field = make_field(&mut gid, inner_of_outer.id());
        let outer_tail_apply = make_apply(&mut gid, &outer_type, &[outer_param.id()]);
        let outer_tail = make_field(&mut gid, outer_tail_apply.id());

        let concrete = make_apply(&mut gid, &outer_type, &[&STRING_TYPE]);
        let root_field = make_field(&mut gid, concrete.id());
        let root = Id::new_uuid();

        let path_value = Path::orphan(root.clone())
            .child(root_field.id().clone())
            .child(outer_value.id().clone());
        assert_eq!(expected_type(&gid, &path_value).unwrap().id, STRING_TYPE);

        let path_inner_head = Path::orphan(root.clone())
            .child(root_field.id().clone())
            .child(outer_list_field.id().clone())
            .child(inner_head.id().clone());
        assert_eq!(expected_type(&gid, &path_inner_head).unwrap().id, STRING_TYPE);

        let path_inner_tail_head = Path::orphan(root.clone())
            .child(root_field.id().clone())
            .child(outer_list_field.id().clone())
            .child(inner_tail.id().clone())
            .child(inner_head.id().clone());
        assert_eq!(expected_type(&gid, &path_inner_tail_head).unwrap().id, STRING_TYPE);

        let path_outer_tail_inner = Path::orphan(root)
            .child(root_field.id().clone())
            .child(outer_tail.id().clone())
            .child(outer_list_field.id().clone())
            .child(inner_tail.id().clone())
            .child(inner_head.id().clone());
        assert_eq!(expected_type(&gid, &path_outer_tail_inner).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn no_type_edge_returns_none() {
        let mut gid = MutGid::new();
        let field = Field::new(&mut gid);
        let path = Path::orphan(Id::new_uuid()).child(field.id().clone());
        assert!(expected_type(&gid, &path).is_none());
    }

    #[test]
    fn root_path_returns_none() {
        let gid = MutGid::new();
        let path = Path::orphan(Id::new_uuid());
        assert!(expected_type(&gid, &path).is_none());
    }

    #[test]
    fn non_generic_apply_no_substitution() {
        let mut gid = MutGid::new();
        let base_type = Type::new(&mut gid);
        let record = Record::new(&mut gid);
        base_type.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        let apply = Apply::new(&mut gid);
        apply.set_base(&mut gid, &base_type);

        let apply_field = make_field(&mut gid, apply.id());
        let inner = make_field(&mut gid, &STRING_TYPE);

        let path = Path::orphan(Id::new_uuid())
            .child(apply_field.id().clone())
            .child(inner.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn concrete_type_parent_no_substitution() {
        let mut gid = MutGid::new();
        let outer = make_field(&mut gid, &STRING_TYPE);
        let inner = make_field(&mut gid, &NUMBER_TYPE);

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id().clone())
            .child(inner.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, NUMBER_TYPE);
    }

    #[test]
    fn unresolved_param_returned_as_is() {
        let mut gid = MutGid::new();
        let param_t = Type::new(&mut gid);
        let field = make_field(&mut gid, param_t.id());

        let path = Path::orphan(Id::new_uuid()).child(field.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, *param_t.id());
    }
}
