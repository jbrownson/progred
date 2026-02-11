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
        let empty_uuid = uuid::Uuid::new_v4();
        gid.merge(im::hashmap! {
            empty_uuid => im::hashmap! {
                ISA.clone() => crate::generated::semantics::EMPTY_TYPE.clone(),
            }
        });

        elements.iter().rev().fold(Id::Uuid(empty_uuid), |tail_node, elem| {
            let cons_uuid = uuid::Uuid::new_v4();
            gid.merge(im::hashmap! {
                cons_uuid => im::hashmap! {
                    ISA.clone() => CONS_TYPE.clone(),
                    HEAD.clone() => elem.clone(),
                    TAIL.clone() => tail_node,
                }
            });
            Id::Uuid(cons_uuid)
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

        let uuid1 = uuid::Uuid::new_v4();
        let uuid2 = uuid::Uuid::new_v4();
        let cons1 = Id::Uuid(uuid1);
        let cons2 = Id::Uuid(uuid2);
        gid.merge(im::hashmap! {
            uuid1 => im::hashmap! {
                ISA.clone() => CONS_TYPE.clone(),
                HEAD.clone() => Id::String("a".into()),
                TAIL.clone() => cons2.clone(),
            },
            uuid2 => im::hashmap! {
                ISA.clone() => CONS_TYPE.clone(),
                HEAD.clone() => Id::String("b".into()),
                TAIL.clone() => cons1.clone(), // cycle back
            },
        });

        let result: Vec<_> = ListIter::new(&gid, Some(&cons1)).cloned().collect();
        assert_eq!(result, vec![Id::String("a".into()), Id::String("b".into())]);
    }
}
