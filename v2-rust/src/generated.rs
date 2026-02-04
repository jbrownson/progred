progred_macros::generate_semantics!("semantics.progred");

impl<T> semantics::List<T> {
    pub fn iter<'a>(&'a self, gid: &'a impl crate::graph::Gid) -> impl Iterator<Item = T> + 'a {
        let conv = self.into_T.clone();
        crate::list_iter::ListIter::new(gid, Some(&self.id)).filter_map(move |id| conv(id))
    }
}

#[cfg(test)]
mod tests {
    use super::semantics::*;
    use crate::graph::{Id, MutGid};

    #[test]
    fn generated_types_exist() {
        let _ = Type::TYPE_ID;
        let _ = Forall::TYPE_ID;
        let _ = Apply::TYPE_ID;
        let _ = Sum::TYPE_ID;
        let _ = Record::TYPE_ID;
        let _ = Field::TYPE_ID;
    }

    #[test]
    fn type_accessors_compile() {
        let gid = MutGid::new();
        let t = Type::wrap(Id::new_uuid());

        let _ = t.name(&gid);
        let _ = t.body(&gid);
    }

    #[test]
    fn forall_accessors_compile() {
        let gid = MutGid::new();
        let f = Forall::wrap(Id::new_uuid());

        let _ = f.params(&gid);
        let _ = f.body(&gid);
    }

    #[test]
    fn apply_accessors_compile() {
        let gid = MutGid::new();
        let a = Apply::wrap(Id::new_uuid());

        let _ = a.base(&gid);
        let _ = a.args(&gid);
    }

    #[test]
    fn sum_accessors_compile() {
        let gid = MutGid::new();
        let s = Sum::wrap(Id::new_uuid());

        let _ = s.variants(&gid);
    }

    #[test]
    fn record_accessors_compile() {
        let gid = MutGid::new();
        let r = Record::wrap(Id::new_uuid());

        let _ = r.fields(&gid);
    }

    #[test]
    fn field_accessors_compile() {
        let gid = MutGid::new();
        let f = Field::wrap(Id::new_uuid());

        let _ = f.name(&gid);
        let _ = f.type_(&gid);
    }

    #[test]
    fn name_returns_string() {
        let mut gid = MutGid::new();
        let id = Id::new_uuid();
        let name_field = Id::Uuid(uuid::Uuid::parse_str(NAME).unwrap());
        gid.set(id.clone(), name_field, Id::String("test_type".to_string()));

        let t = Type::wrap(id);
        let name: Option<std::string::String> = t.name(&gid);
        assert_eq!(name, Some("test_type".to_string()));
    }

    #[test]
    fn accessor_types_are_correct() {
        let gid = MutGid::new();
        let t = Type::wrap(Id::new_uuid());
        let f = Field::wrap(Id::new_uuid());

        // Type and Field have name: string
        let _: Option<std::string::String> = t.name(&gid);
        let _: Option<std::string::String> = f.name(&gid);

        // Note: Record and Sum don't have a name field in the schema
        // They only have fields/variants respectively
    }

    #[test]
    fn list_accessor_returns_iterator() {
        let mut gid = MutGid::new();

        // Build a record with a list of fields
        let record_id = Id::new_uuid();
        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let fields_field = Id::Uuid(uuid::Uuid::parse_str(FIELDS).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(TAIL).unwrap());
        let cons_id = Id::Uuid(uuid::Uuid::parse_str(CONS_TYPE).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());
        let record_type_id = Id::Uuid(uuid::Uuid::parse_str(Record::TYPE_ID).unwrap());

        // Create two field nodes
        let field1 = Id::new_uuid();
        let field2 = Id::new_uuid();

        // Build the list: cons(field1, cons(field2, empty))
        let empty = Id::new_uuid();
        gid.set(empty.clone(), isa_field.clone(), empty_id);

        let cons2 = Id::new_uuid();
        gid.set(cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(cons2.clone(), head_field.clone(), field2.clone());
        gid.set(cons2.clone(), tail_field.clone(), empty);

        let cons1 = Id::new_uuid();
        gid.set(cons1.clone(), isa_field.clone(), cons_id);
        gid.set(cons1.clone(), head_field.clone(), field1.clone());
        gid.set(cons1.clone(), tail_field.clone(), cons2);

        // Set up the record
        gid.set(record_id.clone(), isa_field, record_type_id);
        gid.set(record_id.clone(), fields_field, cons1);

        let record = Record::wrap(record_id);
        let list = record.fields(&gid).expect("fields should exist");
        let fields: Vec<Field> = list.iter(&gid).collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id(), &field1);
        assert_eq!(fields[1].id(), &field2);
    }

    #[test]
    fn list_match_works() {
        let mut gid = MutGid::new();

        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(TAIL).unwrap());
        let cons_id = Id::Uuid(uuid::Uuid::parse_str(CONS_TYPE).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());

        let id_converter: std::rc::Rc<dyn Fn(&Id) -> Option<Id>> = std::rc::Rc::new(|id| Some(id.clone()));

        // Test empty list
        let empty = Id::new_uuid();
        gid.set(empty.clone(), isa_field.clone(), empty_id.clone());

        let list: List<Id> = List::wrap(empty, id_converter.clone());
        let result = list.match_(
            &gid,
            || "empty".to_string(),
            |_, _| "cons".to_string(),
        );
        assert_eq!(result, Some("empty".to_string()));

        // Test cons
        let head_val = Id::new_uuid();
        let cons = Id::new_uuid();
        gid.set(cons.clone(), isa_field.clone(), cons_id);
        gid.set(cons.clone(), head_field, head_val.clone());
        gid.set(cons.clone(), tail_field, empty_id);

        let list: List<Id> = List::wrap(cons, id_converter);
        let result = list.match_(
            &gid,
            || None,
            |h: Id, _| Some(h.clone()),
        );
        assert_eq!(result, Some(Some(head_val)));
    }

    #[test]
    fn list_of_strings() {
        let mut gid = MutGid::new();

        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(TAIL).unwrap());
        let cons_id = Id::Uuid(uuid::Uuid::parse_str(CONS_TYPE).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());

        // Build list: ["hello", "world"]
        let empty = Id::new_uuid();
        gid.set(empty.clone(), isa_field.clone(), empty_id);

        let cons2 = Id::new_uuid();
        gid.set(cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(cons2.clone(), head_field.clone(), Id::String("world".to_string()));
        gid.set(cons2.clone(), tail_field.clone(), empty);

        let cons1 = Id::new_uuid();
        gid.set(cons1.clone(), isa_field.clone(), cons_id);
        gid.set(cons1.clone(), head_field.clone(), Id::String("hello".to_string()));
        gid.set(cons1.clone(), tail_field.clone(), cons2);

        let string_conv: std::rc::Rc<dyn Fn(&Id) -> Option<std::string::String>> = std::rc::Rc::new(|id| {
            match id {
                Id::String(s) => Some(s.clone()),
                _ => None,
            }
        });

        let list: List<std::string::String> = List::wrap(cons1, string_conv);
        let items: Vec<std::string::String> = list.iter(&gid).collect();
        assert_eq!(items, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn nested_list_of_strings() {
        let mut gid = MutGid::new();

        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(TAIL).unwrap());
        let cons_id = Id::Uuid(uuid::Uuid::parse_str(CONS_TYPE).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());

        // Build inner list 1: ["a", "b"]
        let empty1 = Id::new_uuid();
        gid.set(empty1.clone(), isa_field.clone(), empty_id.clone());

        let inner1_cons2 = Id::new_uuid();
        gid.set(inner1_cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(inner1_cons2.clone(), head_field.clone(), Id::String("b".to_string()));
        gid.set(inner1_cons2.clone(), tail_field.clone(), empty1);

        let inner1_cons1 = Id::new_uuid();
        gid.set(inner1_cons1.clone(), isa_field.clone(), cons_id.clone());
        gid.set(inner1_cons1.clone(), head_field.clone(), Id::String("a".to_string()));
        gid.set(inner1_cons1.clone(), tail_field.clone(), inner1_cons2);

        // Build inner list 2: ["x", "y", "z"]
        let empty2 = Id::new_uuid();
        gid.set(empty2.clone(), isa_field.clone(), empty_id.clone());

        let inner2_cons3 = Id::new_uuid();
        gid.set(inner2_cons3.clone(), isa_field.clone(), cons_id.clone());
        gid.set(inner2_cons3.clone(), head_field.clone(), Id::String("z".to_string()));
        gid.set(inner2_cons3.clone(), tail_field.clone(), empty2);

        let inner2_cons2 = Id::new_uuid();
        gid.set(inner2_cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(inner2_cons2.clone(), head_field.clone(), Id::String("y".to_string()));
        gid.set(inner2_cons2.clone(), tail_field.clone(), inner2_cons3);

        let inner2_cons1 = Id::new_uuid();
        gid.set(inner2_cons1.clone(), isa_field.clone(), cons_id.clone());
        gid.set(inner2_cons1.clone(), head_field.clone(), Id::String("x".to_string()));
        gid.set(inner2_cons1.clone(), tail_field.clone(), inner2_cons2);

        // Build outer list: [inner1, inner2]
        let outer_empty = Id::new_uuid();
        gid.set(outer_empty.clone(), isa_field.clone(), empty_id.clone());

        let outer_cons2 = Id::new_uuid();
        gid.set(outer_cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(outer_cons2.clone(), head_field.clone(), inner2_cons1.clone());
        gid.set(outer_cons2.clone(), tail_field.clone(), outer_empty);

        let outer_cons1 = Id::new_uuid();
        gid.set(outer_cons1.clone(), isa_field.clone(), cons_id.clone());
        gid.set(outer_cons1.clone(), head_field.clone(), inner1_cons1.clone());
        gid.set(outer_cons1.clone(), tail_field.clone(), outer_cons2);

        // Create List<List<std::string::String>>
        let string_conv: std::rc::Rc<dyn Fn(&Id) -> Option<std::string::String>> = std::rc::Rc::new(|id| {
            match id {
                Id::String(s) => Some(s.clone()),
                _ => None,
            }
        });

        let inner_list_conv: std::rc::Rc<dyn Fn(&Id) -> Option<List<std::string::String>>> = std::rc::Rc::new(move |id| {
            Some(List::wrap(id.clone(), string_conv.clone()))
        });

        let outer_list: List<List<std::string::String>> = List::wrap(outer_cons1, inner_list_conv);

        // Iterate outer list and collect inner lists' contents
        let result: Vec<Vec<std::string::String>> = outer_list
            .iter(&gid)
            .map(|inner| inner.iter(&gid).collect())
            .collect();

        assert_eq!(result, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ]);
    }

    #[test]
    fn nested_list_with_type_param_simulation() {
        // This simulates what would happen with List<List<T>> where T is a type param
        // The key is that the inner converter is passed through
        let mut gid = MutGid::new();

        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(TAIL).unwrap());
        let cons_id = Id::Uuid(uuid::Uuid::parse_str(CONS_TYPE).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());

        // Build inner list: [1, 2, 3] (as Numbers)
        let empty = Id::new_uuid();
        gid.set(empty.clone(), isa_field.clone(), empty_id.clone());

        let cons3 = Id::new_uuid();
        gid.set(cons3.clone(), isa_field.clone(), cons_id.clone());
        gid.set(cons3.clone(), head_field.clone(), Id::Number(ordered_float::OrderedFloat(3.0)));
        gid.set(cons3.clone(), tail_field.clone(), empty);

        let cons2 = Id::new_uuid();
        gid.set(cons2.clone(), isa_field.clone(), cons_id.clone());
        gid.set(cons2.clone(), head_field.clone(), Id::Number(ordered_float::OrderedFloat(2.0)));
        gid.set(cons2.clone(), tail_field.clone(), cons3);

        let cons1 = Id::new_uuid();
        gid.set(cons1.clone(), isa_field.clone(), cons_id.clone());
        gid.set(cons1.clone(), head_field.clone(), Id::Number(ordered_float::OrderedFloat(1.0)));
        gid.set(cons1.clone(), tail_field.clone(), cons2);

        // Build outer list: [inner]
        let outer_empty = Id::new_uuid();
        gid.set(outer_empty.clone(), isa_field.clone(), empty_id.clone());

        let outer_cons = Id::new_uuid();
        gid.set(outer_cons.clone(), isa_field.clone(), cons_id.clone());
        gid.set(outer_cons.clone(), head_field.clone(), cons1.clone());
        gid.set(outer_cons.clone(), tail_field.clone(), outer_empty);

        // This simulates a generic type that takes a converter for T
        // and creates a List<List<T>> using that converter
        fn make_nested_list<T: 'static>(
            id: Id,
            element_conv: std::rc::Rc<dyn Fn(&Id) -> Option<T>>,
        ) -> List<List<T>> {
            let inner_conv: std::rc::Rc<dyn Fn(&Id) -> Option<List<T>>> = std::rc::Rc::new(move |id| {
                Some(List::wrap(id.clone(), element_conv.clone()))
            });
            List::wrap(id, inner_conv)
        }

        let number_conv: std::rc::Rc<dyn Fn(&Id) -> Option<f64>> = std::rc::Rc::new(|id| {
            match id {
                Id::Number(n) => Some(n.0),
                _ => None,
            }
        });

        let nested = make_nested_list(outer_cons, number_conv);
        let result: Vec<Vec<f64>> = nested
            .iter(&gid)
            .map(|inner| inner.iter(&gid).collect())
            .collect();

        assert_eq!(result, vec![vec![1.0, 2.0, 3.0]]);
    }

    #[test]
    fn generic_wrapper_clone() {
        // Test that generic wrappers can be cloned (Rc clone)
        let mut gid = MutGid::new();

        let isa_field = Id::Uuid(uuid::Uuid::parse_str(ISA).unwrap());
        let empty_id = Id::Uuid(uuid::Uuid::parse_str(EMPTY_TYPE).unwrap());

        let empty = Id::new_uuid();
        gid.set(empty.clone(), isa_field, empty_id);

        let conv: std::rc::Rc<dyn Fn(&Id) -> Option<std::string::String>> = std::rc::Rc::new(|id| {
            match id {
                Id::String(s) => Some(s.clone()),
                _ => None,
            }
        });

        let list1: List<std::string::String> = List::wrap(empty, conv);
        let list2 = list1.clone();

        // Both should work
        assert_eq!(list1.iter(&gid).count(), 0);
        assert_eq!(list2.iter(&gid).count(), 0);
    }
}
