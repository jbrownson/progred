//! Document files: the graph as pretty JSON on disk, format-tagged.
//! Write-through editing means the graph is always current, so saving
//! is a plain serialization of the model. Loaders refuse unknown
//! versions (and untagged pre-version files) rather than guess — a
//! precise refusal today, a migration hook if a file ever matters.

use crate::raw::Document;
use progred_graph::Cells;
use progred_graph::Value;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct FileDoc {
    format: u32,
    root: Option<Value>,
    cells: Cells,
}

pub fn load(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file: FileDoc = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if file.format != FORMAT {
        return Err(format!("format {} (this build reads {FORMAT})", file.format));
    }
    Ok(Document {
        root: file.root,
        cells: file.cells,
    })
}

pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    let file = FileDoc {
        format: FORMAT,
        root: doc.root.clone(),
        cells: doc.cells.clone(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|error| error.to_string())?;
    // Write-then-rename, so a crash mid-write cannot truncate the
    // previous save.
    let tmp = path.with_extension("progred.tmp");
    std::fs::write(&tmp, json).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_round_trip_through_disk_json() {
        let doc = crate::raw::sample_document();
        let path = std::env::temp_dir().join(format!(
            "progred-store-test-{}.progred",
            std::process::id()
        ));
        save(&path, &doc).unwrap();
        let loaded = load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(loaded.root, doc.root);
        assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            serde_json::to_string(&doc).unwrap()
        );
    }

    #[test]
    fn unversioned_and_future_files_refuse() {
        let dir = std::env::temp_dir();
        let old = dir.join(format!("progred-store-old-{}.progred", std::process::id()));
        std::fs::write(&old, r#"{"root": null, "cells": {}}"#).unwrap();
        assert!(load(&old).is_err());
        let future = dir.join(format!("progred-store-new-{}.progred", std::process::id()));
        std::fs::write(&future, r#"{"format": 99, "root": null, "cells": {}}"#).unwrap();
        assert!(load(&future).unwrap_err().contains("99"));
        std::fs::remove_file(&old).ok();
        std::fs::remove_file(&future).ok();
    }
}

