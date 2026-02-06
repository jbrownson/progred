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
    use crate::graph::{Id, MutGid, Gid};

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
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);

        let _ = t.name(&gid);
        let _ = t.body(&gid);
    }

    #[test]
    fn forall_accessors_compile() {
        let mut gid = MutGid::new();
        let f = Forall::new(&mut gid);

        let _ = f.params(&gid);
        let _ = f.body(&gid);
    }

    #[test]
    fn apply_accessors_compile() {
        let mut gid = MutGid::new();
        let a = Apply::new(&mut gid);

        let _ = a.base(&gid);
        let _ = a.args(&gid);
    }

    #[test]
    fn sum_accessors_compile() {
        let mut gid = MutGid::new();
        let s = Sum::new(&mut gid);

        let _ = s.variants(&gid);
    }

    #[test]
    fn record_accessors_compile() {
        let mut gid = MutGid::new();
        let r = Record::new(&mut gid);

        let _ = r.fields(&gid);
    }

    #[test]
    fn field_accessors_compile() {
        let mut gid = MutGid::new();
        let f = Field::new(&mut gid);

        let _ = f.name(&gid);
        let _ = f.type_(&gid);
    }

    #[test]
    fn name_returns_string() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        t.set_name(&mut gid, "test_type");

        let name: Option<std::string::String> = t.name(&gid);
        assert_eq!(name, Some("test_type".to_string()));
    }

    #[test]
    fn accessor_types_are_correct() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);
        let f = Field::new(&mut gid);

        // Type and Field have name: string
        let _: Option<std::string::String> = t.name(&gid);
        let _: Option<std::string::String> = f.name(&gid);
    }

    fn field_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<Field>> {
        std::rc::Rc::new(|id| Some(Field::wrap(id.clone())))
    }

    #[test]
    fn list_accessor_returns_iterator() {
        let mut gid = MutGid::new();

        // Create two fields
        let field1 = Field::new(&mut gid);
        let field2 = Field::new(&mut gid);

        // Build list: [field1, field2]
        let conv = field_converter();
        let empty = List::new_empty(&mut gid, conv.clone());
        let list2 = List::new_cons(&mut gid, field2.id(), &empty, conv.clone());
        let list1 = List::new_cons(&mut gid, field1.id(), &list2, conv);

        // Set up record with this field list
        let record = Record::new(&mut gid);
        record.set_fields(&mut gid, &list1);

        // Read it back
        let list = record.fields(&gid).expect("fields should exist");
        let fields: Vec<Field> = list.iter(&gid).collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id(), field1.id());
        assert_eq!(fields[1].id(), field2.id());
    }

    fn id_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<Id>> {
        std::rc::Rc::new(|id| Some(id.clone()))
    }

    #[test]
    fn list_match_works() {
        let mut gid = MutGid::new();
        let conv = id_converter();

        // Test empty list
        let empty: List<Id> = List::new_empty(&mut gid, conv.clone());
        let result = empty.match_(
            &gid,
            || "empty".to_string(),
            |_, _| "cons".to_string(),
        );
        assert_eq!(result, Some("empty".to_string()));

        // Test cons
        let head_val = Id::new_uuid();
        let cons: List<Id> = List::new_cons(&mut gid, &head_val, &empty, conv);
        let result = cons.match_(
            &gid,
            || None,
            |h: Id, _| Some(h.clone()),
        );
        assert_eq!(result, Some(Some(head_val)));
    }

    fn string_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<std::string::String>> {
        std::rc::Rc::new(|id| match id {
            Id::String(s) => Some(s.clone()),
            _ => None,
        })
    }

    #[test]
    fn list_of_strings() {
        let mut gid = MutGid::new();
        let conv = string_converter();

        // Build list: ["hello", "world"]
        let empty = List::new_empty(&mut gid, conv.clone());
        let list2 = List::new_cons(&mut gid, &Id::String("world".into()), &empty, conv.clone());
        let list1 = List::new_cons(&mut gid, &Id::String("hello".into()), &list2, conv);

        let items: Vec<std::string::String> = list1.iter(&gid).collect();
        assert_eq!(items, vec!["hello".to_string(), "world".to_string()]);
    }

    fn inner_list_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<List<std::string::String>>> {
        let string_conv = string_converter();
        std::rc::Rc::new(move |id| Some(List::wrap(id.clone(), string_conv.clone())))
    }

    #[test]
    fn nested_list_of_strings() {
        let mut gid = MutGid::new();
        let str_conv = string_converter();

        // Build inner list 1: ["a", "b"]
        let empty1 = List::new_empty(&mut gid, str_conv.clone());
        let inner1_b = List::new_cons(&mut gid, &Id::String("b".into()), &empty1, str_conv.clone());
        let inner1 = List::new_cons(&mut gid, &Id::String("a".into()), &inner1_b, str_conv.clone());

        // Build inner list 2: ["x", "y", "z"]
        let empty2 = List::new_empty(&mut gid, str_conv.clone());
        let inner2_z = List::new_cons(&mut gid, &Id::String("z".into()), &empty2, str_conv.clone());
        let inner2_y = List::new_cons(&mut gid, &Id::String("y".into()), &inner2_z, str_conv.clone());
        let inner2 = List::new_cons(&mut gid, &Id::String("x".into()), &inner2_y, str_conv);

        // Build outer list: [inner1, inner2]
        let outer_conv = inner_list_converter();
        let outer_empty = List::new_empty(&mut gid, outer_conv.clone());
        let outer2 = List::new_cons(&mut gid, inner2.id(), &outer_empty, outer_conv.clone());
        let outer = List::new_cons(&mut gid, inner1.id(), &outer2, outer_conv);

        let result: Vec<Vec<std::string::String>> = outer
            .iter(&gid)
            .map(|inner| inner.iter(&gid).collect())
            .collect();

        assert_eq!(result, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ]);
    }

    fn number_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<f64>> {
        std::rc::Rc::new(|id| match id {
            Id::Number(n) => Some(n.0),
            _ => None,
        })
    }

    #[test]
    fn nested_list_with_type_param_simulation() {
        let mut gid = MutGid::new();
        let num_conv = number_converter();

        // Build inner list: [1, 2, 3]
        let empty = List::new_empty(&mut gid, num_conv.clone());
        let inner3 = List::new_cons(&mut gid, &Id::Number(ordered_float::OrderedFloat(3.0)), &empty, num_conv.clone());
        let inner2 = List::new_cons(&mut gid, &Id::Number(ordered_float::OrderedFloat(2.0)), &inner3, num_conv.clone());
        let inner = List::new_cons(&mut gid, &Id::Number(ordered_float::OrderedFloat(1.0)), &inner2, num_conv);

        // Build outer list containing the inner list
        fn make_nested_list_conv<T: 'static>(
            element_conv: std::rc::Rc<dyn Fn(&Id) -> Option<T>>,
        ) -> std::rc::Rc<dyn Fn(&Id) -> Option<List<T>>> {
            std::rc::Rc::new(move |id| Some(List::wrap(id.clone(), element_conv.clone())))
        }

        let outer_conv = make_nested_list_conv(number_converter());
        let outer_empty = List::new_empty(&mut gid, outer_conv.clone());
        let outer = List::new_cons(&mut gid, inner.id(), &outer_empty, outer_conv);

        let result: Vec<Vec<f64>> = outer
            .iter(&gid)
            .map(|inner| inner.iter(&gid).collect())
            .collect();

        assert_eq!(result, vec![vec![1.0, 2.0, 3.0]]);
    }

    #[test]
    fn generic_wrapper_clone() {
        let mut gid = MutGid::new();

        let list1: List<std::string::String> = List::new_empty(&mut gid, string_converter());
        let list2 = list1.clone();

        // Both should work
        assert_eq!(list1.iter(&gid).count(), 0);
        assert_eq!(list2.iter(&gid).count(), 0);
    }

    #[test]
    fn constructor_sets_isa() {
        let mut gid = MutGid::new();
        let t = Type::new(&mut gid);

        let isa = gid.get(t.id(), &ISA);
        assert_eq!(isa, Some(&Type::TYPE_ID));
    }

    #[test]
    fn setter_for_reference_field() {
        let mut gid = MutGid::new();

        // Create a Record and wrap as TypeExpression body
        let record = Record::new(&mut gid);
        let t = Type::new(&mut gid);

        gid.set(t.id().clone(), BODY.clone(), record.id().clone());

        // Read it back - verify the id matches
        let body = t.body(&gid);
        assert!(body.is_some());
        assert_eq!(body.unwrap().id(), record.id());
    }

    #[test]
    fn build_record_with_fields() {
        let mut gid = MutGid::new();

        // Create fields
        let name_field = Field::new(&mut gid);
        name_field.set_name(&mut gid, "name");

        let age_field = Field::new(&mut gid);
        age_field.set_name(&mut gid, "age");

        // Create field list
        let conv = field_converter();
        let empty = List::new_empty(&mut gid, conv.clone());
        let list1 = List::new_cons(&mut gid, age_field.id(), &empty, conv.clone());
        let fields = List::new_cons(&mut gid, name_field.id(), &list1, conv);

        // Create record with fields
        let record = Record::new(&mut gid);
        record.set_fields(&mut gid, &fields);

        // Verify
        let field_list = record.fields(&gid).unwrap();
        let field_vec: Vec<Field> = field_list.iter(&gid).collect();
        assert_eq!(field_vec.len(), 2);
        assert_eq!(field_vec[0].name(&gid), Some("name".to_string()));
        assert_eq!(field_vec[1].name(&gid), Some("age".to_string()));
    }

    #[test]
    fn field_type_reference() {
        let mut gid = MutGid::new();

        // Create a Type to reference (Type is a valid TypeExpression variant)
        let ref_type = Type::new(&mut gid);
        ref_type.set_name(&mut gid, "String");

        let field = Field::new(&mut gid);
        field.set_name(&mut gid, "title");
        gid.set(field.id().clone(), TYPE_.clone(), ref_type.id().clone());

        // Read it back - verify id matches
        assert_eq!(field.name(&gid), Some("title".to_string()));
        let field_type = field.type_(&gid).unwrap();
        assert_eq!(field_type.id(), ref_type.id());
    }

    #[test]
    fn list_constructors_match() {
        let mut gid = MutGid::new();
        let conv = string_converter();

        // Build with constructors
        let empty = List::new_empty(&mut gid, conv.clone());
        let list = List::new_cons(&mut gid, &Id::String("hello".into()), &empty, conv);

        // Verify via match_
        let is_cons = list.match_(
            &gid,
            || false,
            |head, _tail| head == "hello",
        );
        assert_eq!(is_cons, Some(true));
    }

    #[test]
    fn empty_list_match() {
        let mut gid = MutGid::new();
        let empty: List<std::string::String> = List::new_empty(&mut gid, string_converter());

        let is_empty = empty.match_(
            &gid,
            || true,
            |_, _| false,
        );
        assert_eq!(is_empty, Some(true));
    }

    fn type_expr_converter() -> std::rc::Rc<dyn Fn(&Id) -> Option<TypeExpression>> {
        std::rc::Rc::new(|id| Some(TypeExpression::wrap(id.clone())))
    }

    #[test]
    fn forall_with_params() {
        let mut gid = MutGid::new();

        // Create a Type as a parameter (Type is a TypeExpression variant)
        let param = Type::new(&mut gid);
        param.set_name(&mut gid, "T");

        // Create param list using raw graph (since we need List<TypeExpression>)
        let conv = type_expr_converter();
        let empty = List::new_empty(&mut gid, conv.clone());
        let params = List::new_cons(&mut gid, param.id(), &empty, conv);

        let forall = Forall::new(&mut gid);
        gid.set(forall.id().clone(), PARAMS.clone(), params.id().clone());

        // Verify
        let param_list = forall.params(&gid).unwrap();
        let params_vec: Vec<TypeExpression> = param_list.iter(&gid).collect();
        assert_eq!(params_vec.len(), 1);
        assert_eq!(params_vec[0].id(), param.id());
    }
}
