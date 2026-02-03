use crate::generated::semantics::{CONS_TYPE, HEAD, ISA, TAIL};
use crate::graph::{Gid, Id};
use std::collections::HashSet;

fn id(s: &str) -> Id {
    Id::Uuid(uuid::Uuid::parse_str(s).unwrap())
}

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
        let isa_field = id(ISA);
        let cons_id = id(CONS_TYPE);
        let head_field = id(HEAD);
        let tail_field = id(TAIL);

        loop {
            let current = self.current?;

            if self.gid.get(current, &isa_field) != Some(&cons_id) {
                self.current = None;
                return None;
            }

            if !self.seen.insert(current.clone()) {
                self.current = None;
                return None;
            }

            let head = self.gid.get(current, &head_field);
            self.current = self.gid.get(current, &tail_field);

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
        let isa = id(ISA);
        let head = id(HEAD);
        let tail = id(TAIL);
        let cons = id(CONS_TYPE);
        let empty = id(crate::generated::semantics::EMPTY_TYPE);

        let empty_node = Id::new_uuid();
        gid.set(empty_node.clone(), isa.clone(), empty);

        elements.iter().rev().fold(empty_node, |tail_node, elem| {
            let cons_node = Id::new_uuid();
            gid.set(cons_node.clone(), isa.clone(), cons.clone());
            gid.set(cons_node.clone(), head.clone(), elem.clone());
            gid.set(cons_node.clone(), tail.clone(), tail_node);
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
        let isa = id(ISA);
        let head_f = id(HEAD);
        let tail_f = id(TAIL);
        let cons = id(CONS_TYPE);

        let cons1 = Id::new_uuid();
        let cons2 = Id::new_uuid();
        gid.set(cons1.clone(), isa.clone(), cons.clone());
        gid.set(cons1.clone(), head_f.clone(), Id::String("a".into()));
        gid.set(cons1.clone(), tail_f.clone(), cons2.clone());
        gid.set(cons2.clone(), isa.clone(), cons);
        gid.set(cons2.clone(), head_f.clone(), Id::String("b".into()));
        gid.set(cons2.clone(), tail_f, cons1.clone()); // cycle back

        let result: Vec<_> = ListIter::new(&gid, Some(&cons1)).cloned().collect();
        assert_eq!(result, vec![Id::String("a".into()), Id::String("b".into())]);
    }
}
