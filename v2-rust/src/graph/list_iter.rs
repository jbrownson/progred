// Used by proc-macro generated code in crate::generated::semantics
#![allow(dead_code)]

use super::gid::Gid;
use super::id::Id;
use std::collections::HashSet;

pub struct ListIter<'a, G: Gid> {
    gid: &'a G,
    current: Option<&'a Id>,
    isa_field: Id,
    cons_id: Id,
    head_field: Id,
    tail_field: Id,
    seen: HashSet<Id>,
}

impl<'a, G: Gid> ListIter<'a, G> {
    pub fn new(
        gid: &'a G,
        list_node: Option<&'a Id>,
        isa_field: Id,
        cons_id: Id,
        head_field: Id,
        tail_field: Id,
    ) -> Self {
        Self {
            gid,
            current: list_node,
            isa_field,
            cons_id,
            head_field,
            tail_field,
            seen: HashSet::new(),
        }
    }
}

impl<'a, G: Gid> Iterator for ListIter<'a, G> {
    type Item = &'a Id;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.current?;

            if self.gid.get(current, &self.isa_field) != Some(&self.cons_id) {
                self.current = None;
                return None;
            }

            if !self.seen.insert(current.clone()) {
                self.current = None;
                return None;
            }

            let head = self.gid.get(current, &self.head_field);
            self.current = self.gid.get(current, &self.tail_field);

            if head.is_some() {
                return head;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::MutGid;

    fn uuid(n: u128) -> Id {
        Id::Uuid(uuid::Uuid::from_u128(n))
    }

    const ISA: u128 = 1;
    const CONS: u128 = 2;
    const EMPTY: u128 = 3;
    const HEAD: u128 = 4;
    const TAIL: u128 = 5;

    fn make_list(gid: &mut MutGid, elements: &[Id]) -> Id {
        let empty = uuid(100);
        gid.set(empty.clone(), uuid(ISA), uuid(EMPTY));

        elements.iter().rev().fold(empty, |tail, elem| {
            let cons = Id::new_uuid();
            gid.set(cons.clone(), uuid(ISA), uuid(CONS));
            gid.set(cons.clone(), uuid(HEAD), elem.clone());
            gid.set(cons.clone(), uuid(TAIL), tail);
            cons
        })
    }

    fn iter_list<'a>(gid: &'a MutGid, list: Option<&'a Id>) -> ListIter<'a, MutGid> {
        ListIter::new(gid, list, uuid(ISA), uuid(CONS), uuid(HEAD), uuid(TAIL))
    }

    #[test]
    fn empty_list() {
        let mut gid = MutGid::new();
        let list = make_list(&mut gid, &[]);
        let result: Vec<_> = iter_list(&gid, Some(&list)).cloned().collect();
        assert!(result.is_empty());
    }

    #[test]
    fn single_element() {
        let mut gid = MutGid::new();
        let elem = Id::String("hello".into());
        let list = make_list(&mut gid, &[elem.clone()]);
        let result: Vec<_> = iter_list(&gid, Some(&list)).cloned().collect();
        assert_eq!(result, vec![elem]);
    }

    #[test]
    fn multiple_elements() {
        let mut gid = MutGid::new();
        let elems = vec![Id::String("a".into()), Id::String("b".into()), Id::String("c".into())];
        let list = make_list(&mut gid, &elems);
        let result: Vec<_> = iter_list(&gid, Some(&list)).cloned().collect();
        assert_eq!(result, elems);
    }

    #[test]
    fn cycle_detection() {
        let mut gid = MutGid::new();
        let cons1 = uuid(200);
        let cons2 = uuid(201);
        gid.set(cons1.clone(), uuid(ISA), uuid(CONS));
        gid.set(cons1.clone(), uuid(HEAD), Id::String("a".into()));
        gid.set(cons1.clone(), uuid(TAIL), cons2.clone());
        gid.set(cons2.clone(), uuid(ISA), uuid(CONS));
        gid.set(cons2.clone(), uuid(HEAD), Id::String("b".into()));
        gid.set(cons2.clone(), uuid(TAIL), cons1.clone()); // cycle back to cons1

        let result: Vec<_> = iter_list(&gid, Some(&cons1)).cloned().collect();
        assert_eq!(result, vec![Id::String("a".into()), Id::String("b".into())]);
    }

    #[test]
    fn none_input() {
        let gid = MutGid::new();
        let result: Vec<_> = iter_list(&gid, None).collect();
        assert!(result.is_empty());
    }
}
