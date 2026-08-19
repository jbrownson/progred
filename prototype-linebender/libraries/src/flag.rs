//! The boolean convention: truth as one byte under the FLAG field,
//! the way text and f64 ride blobs — GID has no boolean atom.

use gid::Value;

pub mod vocabulary {
    use gid::CellId;

    pub const FLAG: CellId = CellId::from_u128(0x9b7e2d40c1a35f68b4d90a72e6153c8d);
}

pub fn value(flag: bool) -> Value {
    Value::record([(vocabulary::FLAG, Value::from(vec![u8::from(flag)]))])
}

pub fn read(value: &Value) -> Option<bool> {
    match value.as_record()?.get(&vocabulary::FLAG)?.as_blob()? {
        [0] => Some(false),
        [1] => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip_and_junk_reads_none() {
        assert_eq!(read(&value(true)), Some(true));
        assert_eq!(read(&value(false)), Some(false));
        assert_eq!(read(&Value::from(vec![1u8])), None);
        assert_eq!(
            read(&Value::record([(
                vocabulary::FLAG,
                Value::from(vec![2u8])
            )])),
            None
        );
    }
}
