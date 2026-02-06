use crate::generated::semantics::{CONS_TYPE, HEAD, ISA, TAIL};
use crate::graph::{Gid, Id};
use std::collections::HashSet;

pub struct ListIter<'a, G: Gid> {
    gid: &'a G,
    current: Option<&'a Id>,
    seen: HashSet<Id>,
}

impl<'a, G: Gid> ListIter<'a, G> {
    pub fn new(gid: &'a G, list_node: Option<&'a Id>) -> Self {
        Self {
            gid,
            current: list_node,
            seen: HashSet::new(),
        }
    }
}

impl<'a, G: Gid> Iterator for ListIter<'a, G> {
    type Item = &'a Id;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current = self.current?;

            if self.gid.get(current, &ISA) != Some(&CONS_TYPE) {
                self.current = None;
                return None;
            }

            if !self.seen.insert(current.clone()) {
                self.current = None;
                return None;
            }

            let head = self.gid.get(current, &HEAD);
            self.current = self.gid.get(current, &TAIL);

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

    fn make_list(gid: &mut MutGid, elements: &[Id]) -> Id {
        let empty_node = Id::new_uuid();
        gid.set(empty_node.clone(), ISA.clone(), crate::generated::semantics::EMPTY_TYPE.clone());

        elements.iter().rev().fold(empty_node, |tail_node, elem| {
            let cons_node = Id::new_uuid();
            gid.set(cons_node.clone(), ISA.clone(), CONS_TYPE.clone());
            gid.set(cons_node.clone(), HEAD.clone(), elem.clone());
            gid.set(cons_node.clone(), TAIL.clone(), tail_node);
            cons_node
        })
    }

    #[test]
    fn empty_list() {
        let mut gid = MutGid::new();
        let list = make_list(&mut gid, &[]);
        let result: Vec<_> = ListIter::new(&gid, Some(&list)).cloned().collect();
        assert!(result.is_empty());
    }

    #[test]
    fn single_element() {
        let mut gid = MutGid::new();
        let elem = Id::String("hello".into());
        let list = make_list(&mut gid, &[elem.clone()]);
        let result: Vec<_> = ListIter::new(&gid, Some(&list)).cloned().collect();
        assert_eq!(result, vec![elem]);
    }

    #[test]
    fn multiple_elements() {
        let mut gid = MutGid::new();
        let elems = vec![Id::String("a".into()), Id::String("b".into()), Id::String("c".into())];
        let list = make_list(&mut gid, &elems);
        let result: Vec<_> = ListIter::new(&gid, Some(&list)).cloned().collect();
        assert_eq!(result, elems);
    }

    #[test]
    fn none_input() {
        let gid = MutGid::new();
        let result: Vec<_> = ListIter::new(&gid, None).collect();
        assert!(result.is_empty());
    }

    #[test]
    fn cycle_detection() {
        let mut gid = MutGid::new();

        let cons1 = Id::new_uuid();
        let cons2 = Id::new_uuid();
        gid.set(cons1.clone(), ISA.clone(), CONS_TYPE.clone());
        gid.set(cons1.clone(), HEAD.clone(), Id::String("a".into()));
        gid.set(cons1.clone(), TAIL.clone(), cons2.clone());
        gid.set(cons2.clone(), ISA.clone(), CONS_TYPE.clone());
        gid.set(cons2.clone(), HEAD.clone(), Id::String("b".into()));
        gid.set(cons2.clone(), TAIL.clone(), cons1.clone()); // cycle back

        let result: Vec<_> = ListIter::new(&gid, Some(&cons1)).cloned().collect();
        assert_eq!(result, vec![Id::String("a".into()), Id::String("b".into())]);
    }
}
