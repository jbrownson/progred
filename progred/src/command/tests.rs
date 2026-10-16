use super::*;

#[test]
fn example_file_stems_name_their_bundled_documents() {
    for example in Example::ALL {
        let path = format!("../examples/{}.gid", example.file_stem());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            example.source(),
            "{path}"
        );
    }
}
