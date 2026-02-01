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
}
