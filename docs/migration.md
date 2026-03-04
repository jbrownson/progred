# Migrating semantics.progred

When the `Document` struct or graph schema changes, `semantics.progred` needs to be migrated. Do this with a temporary Rust binary that uses the project's own types.

## Procedure

1. **Write `src/bin/migrate.rs`** that depends on `progred_core` and `progred_graph`. Deserialize the old format, transform using `MutGid::set()` / `MutGid::delete()` / etc., serialize the new format.

2. **Run before changing the struct.** The migration binary must compile against the *current* `Document` struct to read the old format. If the struct has already changed, temporarily revert it or manually deserialize via `serde_json::Value`.

3. **Use `serde_json::to_string_pretty(&doc)`** to write back — this matches the app's Save output exactly (sorted `BTreeMap` keys, consistent spacing). Never use Python/jq/manual JSON manipulation; the serde `Serialize` impl on `MutGid` sorts edges deterministically.

4. **Delete the binary after use.** It's a one-shot tool, not a permanent part of the codebase.

## Example

```rust
// src/bin/migrate.rs
fn main() {
    let path = std::env::args().nth(1).expect("usage: migrate <path>");
    let contents = std::fs::read_to_string(&path).expect("read failed");
    let mut doc: progred_core::document::Document =
        serde_json::from_str(&contents).expect("parse failed");

    // Use progred_core::generated::semantics::{ISA, CONS_TYPE, ...} for known IDs
    // Use doc.gid.set(uuid, label, value) to create nodes
    // Use doc.gid.delete(&uuid, &label) to remove edges

    let json = serde_json::to_string_pretty(&doc).expect("serialize failed");
    std::fs::write(&path, json).expect("write failed");
}
```

Run: `cargo run --bin migrate -- semantics.progred`
