progred_macros::generate_semantics!();

#[cfg(test)]
mod tests {
    use super::semantics::*;
    use crate::graph::{Id, MutGid};

    #[test]
    fn generated_types_exist() {
        let _ = Enum::TYPE_ID;
        let _ = Variant::TYPE_ID;
        let _ = Field::TYPE_ID;
        let _ = List::TYPE_ID;
    }

    #[test]
    fn field_accessors_compile() {
        let gid = MutGid::new();
        let e = Enum::wrap(Id::new_uuid());

        // These should compile if the methods were generated
        let _ = e.name(&gid);
        let _ = e.variants(&gid);
    }

    #[test]
    fn variant_accessors_compile() {
        let gid = MutGid::new();
        let v = Variant::wrap(Id::new_uuid());

        let _ = v.name(&gid);
        let _ = v.fields(&gid);
    }

    #[test]
    fn field_type_accessors_compile() {
        let gid = MutGid::new();
        let f = Field::wrap(Id::new_uuid());

        let _ = f.name(&gid);
        let _ = f.type_(&gid);
    }

    #[test]
    fn name_returns_string() {
        let mut gid = MutGid::new();
        let id = Id::new_uuid();
        let name_field = Id::Uuid(uuid::Uuid::parse_str("38f3aabf-d3a1-4c99-80d6-3de62afec12e").unwrap());
        gid.set(id.clone(), name_field, Id::String("test_name".to_string()));

        let e = Enum::wrap(id);
        let name: Option<String> = e.name(&gid);
        assert_eq!(name, Some("test_name".to_string()));
    }

    #[test]
    fn fields_returns_vec_of_field() {
        let mut gid = MutGid::new();

        // IDs
        let fields_field = Id::Uuid(uuid::Uuid::parse_str("b6ad2f0c-7024-435f-9e1f-3be3a736b973").unwrap());
        let isa_field = Id::Uuid(uuid::Uuid::parse_str("a567ccd2-129d-4e85-a321-537a5a3857fb").unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str("7e5593fb-b17c-4995-b8f4-37496c718ef2").unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str("14c2b086-2f99-4ef1-9d66-2fd5c4e94116").unwrap());
        let cons_variant = Id::Uuid(uuid::Uuid::parse_str("6026b370-d464-42bf-b660-ed3af2464463").unwrap());
        let empty_variant = Id::Uuid(uuid::Uuid::parse_str("024cee20-6439-404e-aa77-a8aeb7e83b06").unwrap());

        // Create two field nodes
        let field1 = Id::new_uuid();
        let field2 = Id::new_uuid();

        // Create list: cons(field1, cons(field2, empty))
        let empty_node = Id::new_uuid();
        gid.set(empty_node.clone(), isa_field.clone(), empty_variant);

        let cons2 = Id::new_uuid();
        gid.set(cons2.clone(), isa_field.clone(), cons_variant.clone());
        gid.set(cons2.clone(), head_field.clone(), field2.clone());
        gid.set(cons2.clone(), tail_field.clone(), empty_node);

        let cons1 = Id::new_uuid();
        gid.set(cons1.clone(), isa_field.clone(), cons_variant.clone());
        gid.set(cons1.clone(), head_field.clone(), field1.clone());
        gid.set(cons1.clone(), tail_field.clone(), cons2);

        // Create variant with fields pointing to the list
        let variant_id = Id::new_uuid();
        gid.set(variant_id.clone(), fields_field, cons1);

        let v = Variant::wrap(variant_id);
        let fields: Vec<Field> = v.fields(&gid);

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id(), &field1);
        assert_eq!(fields[1].id(), &field2);
    }
}
