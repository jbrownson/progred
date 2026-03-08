use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use crate::path::Path;
use im::HashSet;
use std::collections::HashMap;

pub type TypeSubstitutions = HashMap<Id, Id>;

pub fn expected_type(gid: &impl Gid, path: &Path) -> Option<TypeExpression> {
    let (te, _) = expected_type_with_substitutions(gid, path)?;
    if TypeParam::try_wrap(gid, &te.id()).is_some() { return None; }
    Some(te)
}

pub fn expected_type_with_substitutions(
    gid: &impl Gid,
    path: &Path,
) -> Option<(TypeExpression, TypeSubstitutions)> {
    let (parent, label) = path.pop()?;
    let parent_env = expected_type_with_substitutions(gid, &parent);
    let subs = parent_env.as_ref()
        .map(|(te, outer_subs)| substitutions_for_type(gid, &te.id(), outer_subs))
        .unwrap_or_default();
    let raw_type = gid.get(&label, &Field::TYPE_.into())?;
    let resolved = subs.get(raw_type).unwrap_or(raw_type);
    Some((TypeExpression::try_wrap(gid, resolved)?, subs))
}

pub fn substitutions_for_type(
    gid: &impl Gid,
    type_id: &Id,
    outer_subs: &TypeSubstitutions,
) -> TypeSubstitutions {
    Apply::try_wrap(gid, type_id)
        .and_then(|apply| {
            let base = apply.base(gid)?;
            let base_body = base.body(gid)?;
            let params = Forall::try_wrap(gid, &base_body.id())?.params(gid)?;
            let args = apply.args(gid)?;
            Some((params, args))
        })
        .map(|(params, args)| {
            ListIter::new(gid, Some(params.id()))
                .zip(ListIter::new(gid, Some(args.id())))
                .map(|(param, arg)| {
                    (
                        param.clone(),
                        outer_subs.get(arg).cloned().unwrap_or_else(|| arg.clone()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn resolve_record(gid: &impl Gid, type_id: &Id) -> Option<Record> {
    resolve_record_inner(gid, type_id, HashSet::new())
}

fn resolve_record_inner(
    gid: &impl Gid,
    type_id: &Id,
    seen_types: HashSet<Id>,
) -> Option<Record> {
    if seen_types.contains(type_id) {
        return None;
    }

    let seen_types = seen_types.update(type_id.clone());

    Record::try_wrap(gid, type_id)
        .or_else(|| {
            Type::try_wrap(gid, type_id)
                .and_then(|t| t.body(gid))
                .and_then(|body| resolve_record_inner(gid, &body.id(), seen_types.clone()))
        })
        .or_else(|| {
            Forall::try_wrap(gid, type_id)
                .and_then(|forall| forall.body(gid))
                .and_then(|body| resolve_record_inner(gid, &body.id(), seen_types.clone()))
        })
        .or_else(|| {
            Apply::try_wrap(gid, type_id)
                .and_then(|apply| apply.base(gid))
                .and_then(|base| resolve_record_inner(gid, &base.id(), seen_types))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;
    use crate::path::Path;
    use std::rc::Rc;

    fn sem_gid(gid: &MutGid) -> crate::graph::StackedGid<&MutGid, MutGid> {
        crate::graph::StackedGid::new(gid, semantics_gid())
    }

    fn te_conv() -> Rc<dyn Fn(&dyn crate::graph::Gid, &Id) -> Option<TypeExpression>> {
        Rc::new(|gid: &dyn crate::graph::Gid, id| TypeExpression::try_wrap(gid, id))
    }

    fn tp_conv() -> Rc<dyn Fn(&dyn crate::graph::Gid, &Id) -> Option<TypeParam>> {
        Rc::new(|gid: &dyn crate::graph::Gid, id| TypeParam::try_wrap(gid, id))
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
        t.set_body(gid, &TypeExpression::wrap(forall.uuid));
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

    fn make_field(gid: &mut MutGid, type_uuid: uuid::Uuid) -> Field {
        let f = Field::new(gid);
        f.set_type_(gid, &TypeExpression::wrap(type_uuid));
        f
    }

    #[test]
    fn resolve_record_through_type_alias() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let alias = Type::new(&mut gid);
        alias.set_body(&mut gid, &TypeExpression::wrap(record.uuid));

        assert_eq!(resolve_record(&gid, &alias.id()).map(|r| r.id()), Some(record.id()));
    }

    #[test]
    fn resolve_record_through_forall() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let forall = Forall::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.uuid));

        assert_eq!(resolve_record(&gid, &forall.id()).map(|r| r.id()), Some(record.id()));
    }

    #[test]
    fn resolve_record_through_apply() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (container, forall) = make_generic_type(&mut gid, &[&param_t.id()]);
        let record = Record::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.uuid));
        let apply = make_apply(&mut gid, &container, &[&String::TYPE_UUID.into()]);

        assert_eq!(resolve_record(&gid, &apply.id()).map(|r| r.id()), Some(record.id()));
    }

    #[test]
    fn resolve_record_cycle_returns_none() {
        let mut gid = MutGid::new();
        let a = Type::new(&mut gid);
        let b = Type::new(&mut gid);
        a.set_body(&mut gid, &TypeExpression::wrap(b.uuid));
        b.set_body(&mut gid, &TypeExpression::wrap(a.uuid));

        assert!(resolve_record(&gid, &a.id()).is_none());
    }

    #[test]
    fn expected_type_for_field() {
        let mut gid = MutGid::new();
        let field = Field::new(&mut gid);
        field.set_type_(&mut gid, &TypeExpression::wrap(String::TYPE_UUID));
        let path = Path::orphan(Id::new_uuid()).child(field.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), String::TYPE_UUID.into());
    }

    #[test]
    fn type_param_resolved_through_apply() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (box_type, _) = make_generic_type(&mut gid, &[&param_t.id()]);
        let apply = make_apply(&mut gid, &box_type, &[&String::TYPE_UUID.into()]);

        let outer = make_field(&mut gid, apply.uuid);
        let inner = make_field(&mut gid, param_t.uuid);

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id())
            .child(inner.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), String::TYPE_UUID.into());
    }

    #[test]
    fn multiple_type_params() {
        let mut gid = MutGid::new();
        let param_a = TypeParam::new(&mut gid);
        let param_b = TypeParam::new(&mut gid);
        let (pair_type, _) = make_generic_type(&mut gid, &[&param_a.id(), &param_b.id()]);
        let apply = make_apply(&mut gid, &pair_type, &[&String::TYPE_UUID.into(), &Number::TYPE_UUID.into()]);

        let pair_field = make_field(&mut gid, apply.uuid);
        let first = make_field(&mut gid, param_a.uuid);
        let second = make_field(&mut gid, param_b.uuid);

        let root = Id::new_uuid();
        let path_first = Path::orphan(root.clone())
            .child(pair_field.id())
            .child(first.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_first).unwrap().id(), String::TYPE_UUID.into());

        let path_second = Path::orphan(root)
            .child(pair_field.id())
            .child(second.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_second).unwrap().id(), Number::TYPE_UUID.into());
    }

    #[test]
    fn recursive_type_through_self_apply() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (list_type, forall) = make_generic_type(&mut gid, &[&param_t.id()]);

        let head_field = make_field(&mut gid, param_t.uuid);
        let tail_apply = make_apply(&mut gid, &list_type, &[&param_t.id()]);
        let tail_field = make_field(&mut gid, tail_apply.uuid);
        forall.set_body(&mut gid, &TypeExpression::wrap(uuid::Uuid::new_v4()));

        let concrete = make_apply(&mut gid, &list_type, &[&String::TYPE_UUID.into()]);
        let list_field = make_field(&mut gid, concrete.uuid);

        let root = Id::new_uuid();
        let path_head = Path::orphan(root.clone())
            .child(list_field.id())
            .child(head_field.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_head).unwrap().id(), String::TYPE_UUID.into());

        let path_tail_head = Path::orphan(root)
            .child(list_field.id())
            .child(tail_field.id())
            .child(head_field.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_tail_head).unwrap().id(), String::TYPE_UUID.into());
    }

    #[test]
    fn deep_recursive_resolution() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (list_type, _) = make_generic_type(&mut gid, &[&param_t.id()]);

        let head_field = make_field(&mut gid, param_t.uuid);
        let tail_apply = make_apply(&mut gid, &list_type, &[&param_t.id()]);
        let tail_field = make_field(&mut gid, tail_apply.uuid);

        let concrete = make_apply(&mut gid, &list_type, &[&Number::TYPE_UUID.into()]);
        let list_field = make_field(&mut gid, concrete.uuid);

        let path = Path::orphan(Id::new_uuid())
            .child(list_field.id())
            .child(tail_field.id())
            .child(tail_field.id())
            .child(tail_field.id())
            .child(head_field.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), Number::TYPE_UUID.into());
    }

    #[test]
    fn nested_type_param_resolution() {
        let mut gid = MutGid::new();

        let inner_param = TypeParam::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[&inner_param.id()]);

        let outer_param = TypeParam::new(&mut gid);
        let (outer_type, outer_forall) = make_generic_type(&mut gid, &[&outer_param.id()]);
        let inner_apply = make_apply(&mut gid, &inner_type, &[&outer_param.id()]);
        let wrapper_field = make_field(&mut gid, inner_apply.uuid);
        let record = Record::new(&mut gid);
        let field_conv: Rc<dyn Fn(&dyn crate::graph::Gid, &Id) -> Option<Field>> = Rc::new(|gid: &dyn crate::graph::Gid, id| Field::try_wrap(gid, id));
        let empty_fields = List::new_empty(&mut gid, field_conv.clone());
        let fields_list = List::new_cons(&mut gid, &wrapper_field.id(), &empty_fields, field_conv);
        record.set_fields(&mut gid, &fields_list);
        outer_forall.set_body(&mut gid, &TypeExpression::wrap(record.uuid));

        let outer_apply = make_apply(&mut gid, &outer_type, &[&Number::TYPE_UUID.into()]);
        let container_field = make_field(&mut gid, outer_apply.uuid);
        let leaf_field = make_field(&mut gid, inner_param.uuid);

        let path = Path::orphan(Id::new_uuid())
            .child(container_field.id())
            .child(wrapper_field.id())
            .child(leaf_field.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), Number::TYPE_UUID.into());
    }

    #[test]
    fn nested_recursive_types() {
        let mut gid = MutGid::new();

        let inner_param = TypeParam::new(&mut gid);
        let (inner_type, _) = make_generic_type(&mut gid, &[&inner_param.id()]);
        let inner_head = make_field(&mut gid, inner_param.uuid);
        let inner_tail_apply = make_apply(&mut gid, &inner_type, &[&inner_param.id()]);
        let inner_tail = make_field(&mut gid, inner_tail_apply.uuid);

        let outer_param = TypeParam::new(&mut gid);
        let (outer_type, _) = make_generic_type(&mut gid, &[&outer_param.id()]);
        let outer_value = make_field(&mut gid, outer_param.uuid);
        let inner_of_outer = make_apply(&mut gid, &inner_type, &[&outer_param.id()]);
        let outer_list_field = make_field(&mut gid, inner_of_outer.uuid);
        let outer_tail_apply = make_apply(&mut gid, &outer_type, &[&outer_param.id()]);
        let outer_tail = make_field(&mut gid, outer_tail_apply.uuid);

        let concrete = make_apply(&mut gid, &outer_type, &[&String::TYPE_UUID.into()]);
        let root_field = make_field(&mut gid, concrete.uuid);
        let root = Id::new_uuid();

        let path_value = Path::orphan(root.clone())
            .child(root_field.id())
            .child(outer_value.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_value).unwrap().id(), String::TYPE_UUID.into());

        let path_inner_head = Path::orphan(root.clone())
            .child(root_field.id())
            .child(outer_list_field.id())
            .child(inner_head.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_inner_head).unwrap().id(), String::TYPE_UUID.into());

        let path_inner_tail_head = Path::orphan(root.clone())
            .child(root_field.id())
            .child(outer_list_field.id())
            .child(inner_tail.id())
            .child(inner_head.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_inner_tail_head).unwrap().id(), String::TYPE_UUID.into());

        let path_outer_tail_inner = Path::orphan(root)
            .child(root_field.id())
            .child(outer_tail.id())
            .child(outer_list_field.id())
            .child(inner_tail.id())
            .child(inner_head.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path_outer_tail_inner).unwrap().id(), String::TYPE_UUID.into());
    }

    #[test]
    fn no_type_edge_returns_none() {
        let mut gid = MutGid::new();
        let field = Field::new(&mut gid);
        let path = Path::orphan(Id::new_uuid()).child(field.id());
        assert!(expected_type(&sem_gid(&gid), &path).is_none());
    }

    #[test]
    fn root_path_returns_none() {
        let gid = MutGid::new();
        let path = Path::orphan(Id::new_uuid());
        assert!(expected_type(&sem_gid(&gid), &path).is_none());
    }

    #[test]
    fn non_generic_apply_no_substitution() {
        let mut gid = MutGid::new();
        let base_type = Type::new(&mut gid);
        let record = Record::new(&mut gid);
        base_type.set_body(&mut gid, &TypeExpression::wrap(record.uuid));

        let apply = Apply::new(&mut gid);
        apply.set_base(&mut gid, &base_type);

        let apply_field = make_field(&mut gid, apply.uuid);
        let inner = make_field(&mut gid, String::TYPE_UUID);

        let path = Path::orphan(Id::new_uuid())
            .child(apply_field.id())
            .child(inner.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), String::TYPE_UUID.into());
    }

    #[test]
    fn concrete_type_parent_no_substitution() {
        let mut gid = MutGid::new();
        let outer = make_field(&mut gid, String::TYPE_UUID);
        let inner = make_field(&mut gid, Number::TYPE_UUID);

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id())
            .child(inner.id());
        assert_eq!(expected_type(&sem_gid(&gid), &path).unwrap().id(), Number::TYPE_UUID.into());
    }

    #[test]
    fn unresolved_param_returns_none() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let field = make_field(&mut gid, param_t.uuid);

        let path = Path::orphan(Id::new_uuid()).child(field.id());
        assert!(expected_type(&sem_gid(&gid), &path).is_none());
    }

    #[test]
    fn generic_field_without_apply_returns_none() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (box_type, _) = make_generic_type(&mut gid, &[&param_t.id()]);

        let outer = make_field(&mut gid, box_type.uuid);
        let inner = make_field(&mut gid, param_t.uuid);

        let path = Path::orphan(Id::new_uuid())
            .child(outer.id())
            .child(inner.id());
        assert!(expected_type(&sem_gid(&gid), &path).is_none());
    }
}
