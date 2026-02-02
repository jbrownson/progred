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
}
