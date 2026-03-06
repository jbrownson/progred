use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use im::HashSet;

fn tri_any(iter: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    iter.fold(Some(false), |acc, r| match (acc, r) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (None, _) | (_, None) => None,
        _ => Some(false),
    })
}

pub fn autocomplete_matches(gid: &impl Gid, candidate: &Id, expected: &TypeExpression) -> Option<bool> {
    match candidate {
        Id::String(_) => if expected.id == STRING_TYPE { Some(true) } else { contains_atomic(gid, expected, &STRING_TYPE, HashSet::new()) },
        Id::Number(_) => if expected.id == NUMBER_TYPE { Some(true) } else { contains_atomic(gid, expected, &NUMBER_TYPE, HashSet::new()) },
        Id::Uuid(_) => gid.get(candidate, &ISA)
            .and_then(|isa| isa_matches(gid, isa, expected, HashSet::new())),
    }
}

pub fn isa_autocomplete_matches(gid: &impl Gid, candidate_isa: &Id, expected: &TypeExpression) -> Option<bool> {
    isa_matches(gid, candidate_isa, expected, HashSet::new())
}

pub fn autocomplete_contains_atomic(gid: &impl Gid, expected: &TypeExpression, atomic_type: &Id) -> Option<bool> {
    contains_atomic(gid, expected, atomic_type, HashSet::new())
}

fn isa_matches(gid: &impl Gid, candidate_isa: &Id, expected: &Id, ancestors: HashSet<Id>) -> Option<bool> {
    if ancestors.contains(expected) {
        return None;
    }
    let ancestors = ancestors.update(expected.clone());
    if let Some(t) = Type::try_wrap(gid, expected) {
        if candidate_isa == expected {
            Some(true)
        } else {
            t.body(gid)
                .map_or(Some(false), |body| isa_matches(gid, candidate_isa, &body, ancestors.clone()))
        }
    } else if let Some(sum) = Sum::try_wrap(gid, expected) {
        sum.variants(gid)
            .and_then(|variants| tri_any(
                ListIter::new(gid, Some(&variants))
                    .map(|v| isa_matches(gid, candidate_isa, v, ancestors.clone()))
            ))
    } else if let Some(apply) = Apply::try_wrap(gid, expected) {
        apply.base(gid)
            .and_then(|base| isa_matches(gid, candidate_isa, &base, ancestors))
    } else if let Some(forall) = Forall::try_wrap(gid, expected) {
        forall.body(gid)
            .and_then(|body| isa_matches(gid, candidate_isa, &body, ancestors))
    } else if Record::try_wrap(gid, expected).is_some() {
        Some(candidate_isa == expected)
    } else {
        None
    }
}

fn contains_atomic(gid: &impl Gid, type_id: &Id, atomic_type: &Id, ancestors: HashSet<Id>) -> Option<bool> {
    if type_id == atomic_type {
        return Some(true);
    }
    if ancestors.contains(type_id) {
        return None;
    }
    let ancestors = ancestors.update(type_id.clone());
    if let Some(t) = Type::try_wrap(gid, type_id) {
        t.body(gid)
            .map_or(Some(false), |body| contains_atomic(gid, &body, atomic_type, ancestors.clone()))
    } else if let Some(forall) = Forall::try_wrap(gid, type_id) {
        forall.body(gid)
            .and_then(|body| contains_atomic(gid, &body, atomic_type, ancestors))
    } else if let Some(sum) = Sum::try_wrap(gid, type_id) {
        sum.variants(gid)
            .and_then(|variants| tri_any(
                ListIter::new(gid, Some(&variants))
                    .map(|v| contains_atomic(gid, v, atomic_type, ancestors.clone()))
            ))
    } else if let Some(apply) = Apply::try_wrap(gid, type_id) {
        apply.base(gid)
            .and_then(|base| contains_atomic(gid, &base, atomic_type, ancestors))
    } else if Record::try_wrap(gid, type_id).is_some() {
        Some(false)
    } else {
        None
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;
    use crate::path::Path;
    use crate::type_system::expected_type;

    #[test]
    fn expected_type_for_field() {
        let mut gid = MutGid::new();
        let field = Field::new(&mut gid);
        field.set_type_(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));
        let path = Path::orphan(Id::new_uuid()).child(field.id().clone());
        assert_eq!(expected_type(&gid, &path).unwrap().id, STRING_TYPE);
    }

    #[test]
    fn string_autocompletes_as_string_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(STRING_TYPE.clone());
        assert_eq!(autocomplete_matches(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn number_autocompletes_as_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Number(ordered_float::OrderedFloat(42.0)), &et), Some(true));
    }

    #[test]
    fn string_does_not_autocomplete_as_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        // NUMBER_TYPE isn't recognizable via try_wrap in a bare MutGid (no semantics),
        // so contains_atomic can't introspect it — returns None
        assert_eq!(autocomplete_matches(&gid, &Id::String("hello".into()), &et), None);
    }

    #[test]
    fn uuid_autocompletes_when_isa_matches() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let record = Record::new(&mut gid);
        t.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), t.id().clone());
        let et = TypeExpression::wrap(t.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    fn te_conv() -> std::rc::Rc<dyn Fn(&Id) -> Option<TypeExpression>> {
        std::rc::Rc::new(|id| Some(TypeExpression::wrap(id.clone())))
    }

    fn tp_conv() -> std::rc::Rc<dyn Fn(&Id) -> Option<TypeParam>> {
        std::rc::Rc::new(|id| Some(TypeParam::wrap(id.clone())))
    }

    fn make_generic_type(gid: &mut MutGid, params: &[&Id]) -> (Type, Forall) {
        let conv = tp_conv();
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
        let param_t = TypeParam::new(&mut gid);
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
        let param_a = TypeParam::new(&mut gid);
        let param_b = TypeParam::new(&mut gid);
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
        let param_t = TypeParam::new(&mut gid);
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
        let param_t = TypeParam::new(&mut gid);
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

        let inner_param = TypeParam::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[inner_param.id()]);

        let outer_param = TypeParam::new(&mut gid);
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

        let inner_param = TypeParam::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[inner_param.id()]);
        let inner_head = make_field(&mut gid, inner_param.id());
        let inner_tail_apply = make_apply(&mut gid, &inner_type, &[inner_param.id()]);
        let inner_tail = make_field(&mut gid, inner_tail_apply.id());

        let outer_param = TypeParam::new(&mut gid);
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
    fn unresolved_param_returns_none() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let field = make_field(&mut gid, param_t.id());

        let path = Path::orphan(Id::new_uuid()).child(field.id().clone());
        assert!(expected_type(&gid, &path).is_none());
    }

    #[test]
    fn generic_field_without_apply_returns_none() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (box_type, _) = make_generic_type(&mut gid, &[param_t.id()]);

        // Access a field typed as T directly through the generic type (no Apply to resolve T)
        let outer = make_field(&mut gid, box_type.id());
        let inner = make_field(&mut gid, param_t.id());

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id().clone())
            .child(inner.id().clone());
        assert!(expected_type(&gid, &path).is_none());
    }

    fn make_sum_type(gid: &mut MutGid, variant_types: &[&Type]) -> Type {
        let conv = te_conv();
        let empty = List::new_empty(gid, conv.clone());
        let variants_list = variant_types.iter().rev().fold(empty, |tail, vt| {
            List::new_cons(gid, vt.id(), &tail, conv.clone())
        });
        let sum = Sum::new(gid);
        sum.set_variants(gid, &variants_list);
        let t = Type::new(gid);
        t.set_body(gid, &TypeExpression::wrap(sum.id().clone()));
        t
    }

    #[test]
    fn variant_autocompletes_in_sum() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), dog.id().clone());
        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn non_variant_does_not_autocomplete_in_sum() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let fish = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), fish.id().clone());
        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(false));
    }

    #[test]
    fn isa_body_autocompletes_in_sum_variant() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let dog = Type::new(&mut gid);
        dog.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));
        let cat = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        // ISA points to the Record body, not the Type alias
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), record.id().clone());
        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn isa_body_autocompletes_as_type_alias() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let dog = Type::new(&mut gid);
        dog.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        // ISA points to Record body, expected is the Type alias
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), record.id().clone());
        let et = TypeExpression::wrap(dog.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn uuid_without_isa_indeterminate() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let node_uuid = uuid::Uuid::new_v4();
        // Don't set ISA — can't determine match
        let et = TypeExpression::wrap(t.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), None);
    }

    #[test]
    fn string_autocompletes_in_sum_containing_string() {
        let mut gid = MutGid::new();
        // Type StringWrapper → BODY → STRING_TYPE (alias)
        let string_type = Type::new(&mut gid);
        string_type.set_body(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));
        let number_type = Type::new(&mut gid);
        let mixed = make_sum_type(&mut gid, &[&string_type, &number_type]);

        let et = TypeExpression::wrap(mixed.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn string_does_not_autocomplete_in_sum_without_string() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::String("hello".into()), &et), Some(false));
    }

    #[test]
    fn uuid_autocompletes_through_apply_expected() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (list_type, _) = make_generic_type(&mut gid, &[param_t.id()]);
        let apply = make_apply(&mut gid, &list_type, &[&STRING_TYPE]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), list_type.id().clone());
        let et = TypeExpression::wrap(apply.id().clone());
        // Apply → BASE → list_type, candidate ISA matches list_type
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn uuid_autocompletes_through_forall_expected() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let forall = Forall::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), record.id().clone());
        let et = TypeExpression::wrap(forall.id().clone());
        // Forall → BODY → record, candidate ISA matches record
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn string_autocompletes_through_type_alias() {
        let mut gid = MutGid::new();
        // Type Name = String
        let name_type = Type::new(&mut gid);
        name_type.set_body(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));

        let et = TypeExpression::wrap(name_type.id().clone());
        assert_eq!(autocomplete_matches(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn cyclic_type_body_does_not_stack_overflow() {
        let mut gid = MutGid::new();
        // Type A → BODY → Type B → BODY → Type A
        let a = Type::new(&mut gid);
        let b = Type::new(&mut gid);
        a.set_body(&mut gid, &TypeExpression::wrap(b.id().clone()));
        b.set_body(&mut gid, &TypeExpression::wrap(a.id().clone()));

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), Id::new_uuid());
        let et = TypeExpression::wrap(a.id().clone());
        // Should terminate with None (cycle), not stack overflow
        assert_eq!(autocomplete_matches(&gid, &Id::Uuid(node_uuid), &et), None);
    }

    #[test]
    fn cyclic_type_contains_atomic_does_not_stack_overflow() {
        let mut gid = MutGid::new();
        // Type A → BODY → Type A (self-referential)
        let a = Type::new(&mut gid);
        a.set_body(&mut gid, &TypeExpression::wrap(a.id().clone()));

        let et = TypeExpression::wrap(a.id().clone());
        // Should terminate with None (cycle), not stack overflow
        assert_eq!(autocomplete_contains_atomic(&gid, &et, &STRING_TYPE), None);
    }
}
