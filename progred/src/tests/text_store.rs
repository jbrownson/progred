use super::*;

#[test]
fn replacement_preserves_siblings_and_cleans_up_failed_saves() {
    let directory = std::env::temp_dir().join(format!("progred-store-{}", gid::new_cell_id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("document.gid");
    let sibling = directory.join("document.tmp");
    std::fs::write(&path, "old").unwrap();
    std::fs::write(&sibling, "unrelated").unwrap();

    replace_text(&path, "new").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "unrelated");
    let blocked = directory.join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    assert!(replace_text(&blocked, "cannot replace a directory").is_err());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 3);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn documents_round_trip_through_disk() {
    let doc = crate::sample::sample_document();
    let path = std::env::temp_dir().join(format!("progred-store-test-{}.gid", std::process::id()));
    save(&path, &doc, &Binders::new()).unwrap();
    let (loaded, binders) = load(&path).unwrap();
    assert_eq!(loaded.root, doc.root);
    let again = gid_text::print(&loaded, &binders);
    assert_eq!(again, gid_text::print(&doc, &Binders::new()));
    std::fs::remove_file(&path).ok();
}

#[test]
fn damaged_files_refuse() {
    let path = std::env::temp_dir().join(format!("progred-store-bad-{}.gid", std::process::id()));
    std::fs::write(&path, "{\"root\": ").unwrap();
    assert!(load(&path).is_err());
    std::fs::remove_file(&path).ok();
}
