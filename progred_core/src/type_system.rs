use crate::generated::semantics::*;
use crate::graph::{Gid, Id};
use crate::list_iter::ListIter;
use crate::path::Path;
use im::HashSet;
use std::collections::HashMap;

pub type TypeSubstitutions = HashMap<Id, Id>;

pub fn expected_type(gid: &impl Gid, path: &Path) -> Option<TypeExpression> {
    let (te, _) = expected_type_with_substitutions(gid, path)?;
    if TypeParam::try_wrap(gid, te.id()).is_some() { return None; }
    Some(te)
}

pub fn expected_type_with_substitutions(
    gid: &impl Gid,
    path: &Path,
) -> Option<(TypeExpression, TypeSubstitutions)> {
    let (parent, label) = path.pop()?;
    let parent_env = expected_type_with_substitutions(gid, &parent);
    let subs = parent_env.as_ref()
        .map(|(te, outer_subs)| substitutions_for_type(gid, te.id(), outer_subs))
        .unwrap_or_default();
    let raw_type = gid.get(&label, &TYPE_)?;
    let resolved = subs.get(raw_type).unwrap_or(raw_type);
    Some((TypeExpression::wrap(resolved.clone()), subs))
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
            let params = Forall::try_wrap(gid, base_body.id())?.params(gid)?;
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
                .and_then(|body| resolve_record_inner(gid, body.id(), seen_types.clone()))
        })
        .or_else(|| {
            Forall::try_wrap(gid, type_id)
                .and_then(|forall| forall.body(gid))
                .and_then(|body| resolve_record_inner(gid, body.id(), seen_types.clone()))
        })
        .or_else(|| {
            Apply::try_wrap(gid, type_id)
                .and_then(|apply| apply.base(gid))
                .and_then(|base| resolve_record_inner(gid, base.id(), seen_types))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;
    use std::rc::Rc;

    fn te_conv() -> Rc<dyn Fn(&Id) -> Option<TypeExpression>> {
        Rc::new(|id| Some(TypeExpression::wrap(id.clone())))
    }

    fn tp_conv() -> Rc<dyn Fn(&Id) -> Option<TypeParam>> {
        Rc::new(|id| Some(TypeParam::wrap(id.clone())))
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

    #[test]
    fn resolve_record_through_type_alias() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let alias = Type::new(&mut gid);
        alias.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        assert_eq!(resolve_record(&gid, alias.id()).map(|r| r.id().clone()), Some(record.id().clone()));
    }

    #[test]
    fn resolve_record_through_forall() {
        let mut gid = MutGid::new();
        let record = Record::new(&mut gid);
        let forall = Forall::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));

        assert_eq!(resolve_record(&gid, forall.id()).map(|r| r.id().clone()), Some(record.id().clone()));
    }

    #[test]
    fn resolve_record_through_apply() {
        let mut gid = MutGid::new();
        let param_t = TypeParam::new(&mut gid);
        let (container, forall) = make_generic_type(&mut gid, &[param_t.id()]);
        let record = Record::new(&mut gid);
        forall.set_body(&mut gid, &TypeExpression::wrap(record.id().clone()));
        let apply = make_apply(&mut gid, &container, &[&STRING_TYPE]);

        assert_eq!(resolve_record(&gid, apply.id()).map(|r| r.id().clone()), Some(record.id().clone()));
    }

    #[test]
    fn resolve_record_cycle_returns_none() {
        let mut gid = MutGid::new();
        let a = Type::new(&mut gid);
        let b = Type::new(&mut gid);
        a.set_body(&mut gid, &TypeExpression::wrap(b.id().clone()));
        b.set_body(&mut gid, &TypeExpression::wrap(a.id().clone()));

        assert!(resolve_record(&gid, a.id()).is_none());
    }
}
