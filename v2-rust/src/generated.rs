progred_macros::generate_semantics!();

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
        let name_field = Id::Uuid(uuid::Uuid::parse_str(Field::NAME).unwrap());
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
        let isa_field = Id::Uuid(uuid::Uuid::parse_str(Field::ISA).unwrap());
        let fields_field = Id::Uuid(uuid::Uuid::parse_str(Field::FIELDS).unwrap());
        let head_field = Id::Uuid(uuid::Uuid::parse_str(Field::HEAD).unwrap());
        let tail_field = Id::Uuid(uuid::Uuid::parse_str(Field::TAIL).unwrap());
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
        let fields: Vec<Field> = record.fields(&gid).collect();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id(), &field1);
        assert_eq!(fields[1].id(), &field2);
    }
}
