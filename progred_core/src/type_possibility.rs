use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use crate::type_system::{substitutions_for_type, TypeSubstitutions};
use im::HashSet;

fn tri_any(iter: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    iter.fold(Some(false), |acc, r| match (acc, r) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (None, _) | (_, None) => None,
        _ => Some(false),
    })
}

pub fn type_accepts_candidate(gid: &impl Gid, candidate: &Id, expected: &TypeExpression) -> Option<bool> {
    match candidate {
        Id::String(_) => if expected.id == STRING_TYPE { Some(true) } else { type_may_accept_atomic_inner(gid, expected, &STRING_TYPE, &TypeSubstitutions::new(), HashSet::new()) },
        Id::Number(_) => if expected.id == NUMBER_TYPE { Some(true) } else { type_may_accept_atomic_inner(gid, expected, &NUMBER_TYPE, &TypeSubstitutions::new(), HashSet::new()) },
        Id::Uuid(_) => gid.get(candidate, &ISA)
            .and_then(|isa| type_accepts_isa_inner(gid, isa, expected, HashSet::new())),
    }
}

pub fn type_accepts_isa(gid: &impl Gid, candidate_isa: &Id, expected: &TypeExpression) -> Option<bool> {
    type_accepts_isa_inner(gid, candidate_isa, expected, HashSet::new())
}

pub fn type_may_accept_atomic(gid: &impl Gid, expected: &TypeExpression, atomic_type: &Id) -> Option<bool> {
    type_may_accept_atomic_inner(gid, expected, atomic_type, &TypeSubstitutions::new(), HashSet::new())
}

fn type_accepts_isa_inner(gid: &impl Gid, candidate_isa: &Id, expected: &Id, ancestors: HashSet<Id>) -> Option<bool> {
    if ancestors.contains(expected) {
        return None;
    }
    let ancestors = ancestors.update(expected.clone());
    if let Some(t) = Type::try_wrap(gid, expected) {
        if candidate_isa == expected {
            Some(true)
        } else {
            t.body(gid)
                .map_or(Some(false), |body| type_accepts_isa_inner(gid, candidate_isa, &body, ancestors.clone()))
        }
    } else if let Some(sum) = Sum::try_wrap(gid, expected) {
        sum.variants(gid)
            .and_then(|variants| tri_any(
                ListIter::new(gid, Some(&variants))
                    .map(|v| type_accepts_isa_inner(gid, candidate_isa, v, ancestors.clone()))
            ))
    } else if let Some(apply) = Apply::try_wrap(gid, expected) {
        apply.base(gid)
            .and_then(|base| type_accepts_isa_inner(gid, candidate_isa, &base, ancestors))
    } else if let Some(forall) = Forall::try_wrap(gid, expected) {
        forall.body(gid)
            .and_then(|body| type_accepts_isa_inner(gid, candidate_isa, &body, ancestors))
    } else if Record::try_wrap(gid, expected).is_some() {
        Some(candidate_isa == expected)
    } else {
        None
    }
}

fn type_may_accept_atomic_inner(
    gid: &impl Gid,
    type_id: &Id,
    atomic_type: &Id,
    substitutions: &TypeSubstitutions,
    ancestors: HashSet<Id>,
) -> Option<bool> {
    let type_id = substituted_type_id(substitutions, type_id);
    if type_id == *atomic_type {
        return Some(true);
    }
    if ancestors.contains(&type_id) {
        return None;
    }
    let ancestors = ancestors.update(type_id.clone());
    if let Some(t) = Type::try_wrap(gid, &type_id) {
        t.body(gid)
            .map_or(Some(false), |body| type_may_accept_atomic_inner(gid, body.id(), atomic_type, substitutions, ancestors.clone()))
    } else if let Some(forall) = Forall::try_wrap(gid, &type_id) {
        forall.body(gid)
            .and_then(|body| type_may_accept_atomic_inner(gid, body.id(), atomic_type, substitutions, ancestors))
    } else if let Some(sum) = Sum::try_wrap(gid, &type_id) {
        sum.variants(gid)
            .and_then(|variants| tri_any(
                ListIter::new(gid, Some(&variants))
                    .map(|v| type_may_accept_atomic_inner(gid, v, atomic_type, substitutions, ancestors.clone()))
            ))
    } else if let Some((target, target_substitutions)) = apply_target(gid, &type_id, substitutions) {
        type_may_accept_atomic_inner(gid, &target, atomic_type, &target_substitutions, ancestors)
    } else if Record::try_wrap(gid, &type_id).is_some() {
        Some(false)
    } else {
        None
    }
}

fn substituted_type_id(substitutions: &TypeSubstitutions, type_id: &Id) -> Id {
    substitutions.get(type_id).cloned().unwrap_or_else(|| type_id.clone())
}

fn apply_target(
    gid: &impl Gid,
    type_id: &Id,
    outer_substitutions: &TypeSubstitutions,
) -> Option<(Id, TypeSubstitutions)> {
    let apply = Apply::try_wrap(gid, type_id)?;
    let base = apply.base(gid)?;
    let base_body = base.body(gid);

    if let Some(forall) = base_body.as_ref().and_then(|body| Forall::try_wrap(gid, body.id())) {
        let substitutions = substitutions_for_type(gid, type_id, outer_substitutions);
        let body = forall.body(gid)?;
        Some((substituted_type_id(&substitutions, body.id()), substitutions))
    } else {
        Some((substituted_type_id(outer_substitutions, base.id()), outer_substitutions.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;

    #[test]
    fn string_accepted_as_string_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(STRING_TYPE.clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn number_accepted_as_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Number(ordered_float::OrderedFloat(42.0)), &et), Some(true));
    }

    #[test]
    fn string_not_accepted_as_number_type() {
        let gid = MutGid::new();
        let et = TypeExpression::wrap(NUMBER_TYPE.clone());
        // NUMBER_TYPE isn't recognizable via try_wrap in a bare MutGid (no semantics),
        // so type_may_accept_atomic can't introspect it — returns None
        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), None);
    }

    #[test]
    fn uuid_accepted_when_isa_matches() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let record = Record::new(&mut gid);
        t.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), t.id().clone());
        let et = TypeExpression::wrap(t.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
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
    fn variant_accepted_in_sum() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), dog.id().clone());
        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn non_variant_not_accepted_in_sum() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let fish = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), fish.id().clone());
        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(false));
    }

    #[test]
    fn isa_body_accepted_in_sum_variant() {
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
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn isa_body_accepted_as_type_alias() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let dog = Type::new(&mut gid);
        dog.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        // ISA points to Record body, expected is the Type alias
        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), record.id().clone());
        let et = TypeExpression::wrap(dog.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn uuid_without_isa_indeterminate() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let node_uuid = uuid::Uuid::new_v4();
        // Don't set ISA — can't determine match
        let et = TypeExpression::wrap(t.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), None);
    }

    #[test]
    fn string_accepted_in_sum_containing_string() {
        let mut gid = MutGid::new();
        // Type StringWrapper → BODY → STRING_TYPE (alias)
        let string_type = Type::new(&mut gid);
        string_type.set_body(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));
        let number_type = Type::new(&mut gid);
        let mixed = make_sum_type(&mut gid, &[&string_type, &number_type]);

        let et = TypeExpression::wrap(mixed.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn string_not_accepted_in_sum_without_string() {
        let mut gid = MutGid::new();
        let dog = Type::new(&mut gid);
        let cat = Type::new(&mut gid);
        let animal = make_sum_type(&mut gid, &[&dog, &cat]);

        let et = TypeExpression::wrap(animal.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(false));
    }

    #[test]
    fn uuid_accepted_through_apply_expected() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (list_type, _) = make_generic_type(&mut gid, &[param_t.id()]);
        let apply = make_apply(&mut gid, &list_type, &[&STRING_TYPE]);

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), list_type.id().clone());
        let et = TypeExpression::wrap(apply.id().clone());
        // Apply → BASE → list_type, candidate ISA matches list_type
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn uuid_accepted_through_forall_expected() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let forall = Forall::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        let node_uuid = uuid::Uuid::new_v4();
        gid.set(node_uuid, ISA.clone(), record.id().clone());
        let et = TypeExpression::wrap(forall.id().clone());
        // Forall → BODY → record, candidate ISA matches record
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), Some(true));
    }

    #[test]
    fn string_accepted_through_type_alias() {
        let mut gid = MutGid::new();
        // Type Name = String
        let name_type = Type::new(&mut gid);
        name_type.set_body(&mut gid, &TypeExpression::wrap(STRING_TYPE.clone()));

        let et = TypeExpression::wrap(name_type.id().clone());
        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(true));
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
        assert_eq!(type_accepts_candidate(&gid, &Id::Uuid(node_uuid), &et), None);
    }

    #[test]
    fn cyclic_type_may_accept_atomic_does_not_stack_overflow() {
        let mut gid = MutGid::new();
        // Type A → BODY → Type A (self-referential)
        let a = Type::new(&mut gid);
        a.set_body(&mut gid, &TypeExpression::wrap(a.id().clone()));

        let et = TypeExpression::wrap(a.id().clone());
        // Should terminate with None (cycle), not stack overflow
        assert_eq!(type_may_accept_atomic(&gid, &et, &STRING_TYPE), None);
    }

    #[test]
    fn string_accepted_through_apply_substitution() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (id_type, forall) = make_generic_type(&mut gid, &[param_t.id()]);
        forall.set_body(&mut gid, &TypeExpression::wrap(param_t.id().clone()));

        let applied = make_apply(&mut gid, &id_type, &[&STRING_TYPE]);
        let et = TypeExpression::wrap(applied.id().clone());

        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(true));
    }

    #[test]
    fn string_accepted_through_sum_variant_apply_substitution() {
        let mut gid = MutGid::new();
        let param_a = TypeParam::new(&mut gid);
        let param_b = TypeParam::new(&mut gid);
        let (either_type, forall) = make_generic_type(&mut gid, &[param_a.id(), param_b.id()]);

        let conv = te_conv();
        let empty = List::new_empty(&mut gid, conv.clone());
        let tail = List::new_cons(&mut gid, param_b.id(), &empty, conv.clone());
        let variants = List::new_cons(&mut gid, param_a.id(), &tail, conv);
        let sum = Sum::new(&mut gid);
        sum.set_variants(&mut gid, &variants);
        forall.set_body(&mut gid, &TypeExpression::wrap(sum.id().clone()));

        let applied = make_apply(&mut gid, &either_type, &[&STRING_TYPE, &NUMBER_TYPE]);
        let et = TypeExpression::wrap(applied.id().clone());

        assert_eq!(type_accepts_candidate(&gid, &Id::String("hello".into()), &et), Some(true));
        assert_eq!(type_accepts_candidate(&gid, &Id::Number(ordered_float::OrderedFloat(42.0)), &et), Some(true));
    }
}
