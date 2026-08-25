//! Temporary text import/export for GID documents. This binder notation
//! bridges to text-based tools; it is not GID's native representation.

use gid::{CellId, Cells, Document, Value, new_cell_id};
#[cfg(any(not(target_arch = "wasm32"), test))]
use progred_libraries::name;
use progred_libraries::text;
use std::collections::BTreeMap;
use std::collections::HashSet;
#[cfg(any(not(target_arch = "wasm32"), test))]
use std::fmt::Write as _;

/// The file-local binder table: pure serialization sugar, kept by
/// the store layer between load and save, never part of the model.
pub type Binders = BTreeMap<String, CellId>;

pub fn parse(text: &str) -> Result<(Document, Binders), String> {
    let mut parser = Parser {
        rest: text,
        binders: BTreeMap::new(),
    };
    parser.skip_space();
    let file = parser.file()?;
    parser.skip_space();
    if !parser.rest.is_empty() {
        return Err(parser.err("trailing content after the file record"));
    }
    Ok((file, parser.binders))
}

struct Parser<'a> {
    rest: &'a str,
    binders: Binders,
}

/// A bare token under the two-namespace rule.
enum Token {
    CellId(CellId),
    Binder(String),
}

fn as_cell_id(token: &str) -> Option<CellId> {
    (token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| CellId::parse_str(token).ok())
        .flatten()
}

fn valid_binder(token: &str) -> bool {
    let mut bytes = token.bytes();
    matches!(bytes.next(), Some(b) if b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl Parser<'_> {
    fn err(&self, message: impl std::fmt::Display) -> String {
        let at = self.rest.chars().take(24).collect::<String>();
        format!("{message} (at `{at}…`)")
    }

    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn eat(&mut self, expected: char) -> Result<(), String> {
        self.skip_space();
        match self.rest.strip_prefix(expected) {
            Some(rest) => {
                self.rest = rest;
                Ok(())
            }
            None => Err(self.err(format!("expected `{expected}`"))),
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_space();
        self.rest.chars().next()
    }

    /// The comma-separated interior of `{…}`/`[…]`, trailing comma
    /// optional — one of the defined leniencies.
    fn separated<T>(
        &mut self,
        close: char,
        mut item: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let mut items = Vec::new();
        loop {
            if self.peek() == Some(close) {
                self.rest = &self.rest[1..];
                return Ok(items);
            }
            items.push(item(self)?);
            match self.peek() {
                Some(',') => {
                    self.rest = &self.rest[1..];
                }
                Some(c) if c == close => {}
                _ => return Err(self.err(format!("expected `,` or `{close}`"))),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat('"')?;
        let mut out = String::new();
        let mut chars = self.rest.char_indices();
        loop {
            let Some((at, c)) = chars.next() else {
                return Err("unterminated string".to_string());
            };
            match c {
                '"' => {
                    self.rest = &self.rest[at + 1..];
                    return Ok(out);
                }
                '\n' => return Err("raw newline in string; spell it \\n".to_string()),
                '\\' => match chars.next() {
                    Some((_, '"')) => out.push('"'),
                    Some((_, '\\')) => out.push('\\'),
                    Some((_, 'n')) => out.push('\n'),
                    Some((_, 't')) => out.push('\t'),
                    Some((_, other)) => {
                        return Err(format!("unknown escape `\\{other}`"));
                    }
                    None => return Err("unterminated string".to_string()),
                },
                c => out.push(c),
            }
        }
    }

    fn bare(&mut self) -> Result<&str, String> {
        self.skip_space();
        let end = self
            .rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(self.rest.len());
        if end == 0 {
            return Err(self.err("expected a value"));
        }
        let token = &self.rest[..end];
        self.rest = &self.rest[end..];
        Ok(token)
    }

    /// A bare token as an identity: cell-id literal, or binder minting on first
    /// use — create-on-reference at the file layer.
    fn token(&mut self) -> Result<Token, String> {
        let token = self.bare()?;
        if let Some(cell) = as_cell_id(&token.to_ascii_lowercase()) {
            return Ok(Token::CellId(cell));
        }
        if !valid_binder(token) {
            return Err(format!("`{token}` is neither a cell id nor a binder"));
        }
        Ok(Token::Binder(token.to_string()))
    }

    fn resolve(&mut self, token: Token) -> CellId {
        match token {
            Token::CellId(cell) => cell,
            Token::Binder(binder) => *self.binders.entry(binder).or_insert_with(new_cell_id),
        }
    }

    fn blob(&mut self) -> Result<Vec<u8>, String> {
        self.rest = &self.rest[2..];
        let end = self
            .rest
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(self.rest.len());
        let hex = &self.rest[..end];
        if !hex.len().is_multiple_of(2) {
            return Err("a blob needs whole bytes (even hex digits)".to_string());
        }
        let bytes = hex
            .as_bytes()
            .chunks(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("hex is ascii"), 16)
                    .expect("checked hex")
            })
            .collect();
        self.rest = &self.rest[end..];
        Ok(bytes)
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some('"') => Ok(text::value(self.string()?)),
            Some('{') => {
                self.rest = &self.rest[1..];
                let fields = self.separated('}', |p| {
                    let label = p.label()?;
                    p.eat(':')?;
                    Ok((label, p.value()?))
                })?;
                let mut seen = HashSet::new();
                for (label, _) in &fields {
                    if !seen.insert(*label) {
                        return Err("duplicate label in a record".to_string());
                    }
                }
                Ok(Value::record(fields))
            }
            Some('[') => {
                self.rest = &self.rest[1..];
                let elements = self.separated(']', Self::value)?;
                Ok(Value::list(elements))
            }
            Some('0') if self.rest.as_bytes().get(1) == Some(&b'x') => {
                Ok(Value::from(self.blob()?))
            }
            _ => {
                let token = self.token()?;
                Ok(Value::from(self.resolve(token)))
            }
        }
    }

    fn label(&mut self) -> Result<CellId, String> {
        let token = self.token()?;
        Ok(self.resolve(token))
    }

    /// Fields apply as they parse: `binders` must precede any use of
    /// its binders, since resolution mints as it goes.
    fn file(&mut self) -> Result<Document, String> {
        self.eat('{')?;
        let mut binders_seen = false;
        let mut cells_seen = false;
        let mut stated = HashSet::new();
        let mut cells = Cells::new();
        let mut root = None;
        self.separated('}', |p| {
            let key = p.string()?;
            p.eat(':')?;
            match key.as_str() {
                "binders" => {
                    if binders_seen {
                        return Err("two `binders` tables".to_string());
                    }
                    binders_seen = true;
                    p.eat('{')?;
                    p.separated('}', |p| {
                        let binder = p.string()?;
                        if !valid_binder(&binder) {
                            return Err(format!("`{binder}` is not a usable binder"));
                        }
                        if as_cell_id(&binder.to_ascii_lowercase()).is_some() {
                            return Err(format!("`{binder}` spells a cell id, not a binder"));
                        }
                        p.eat(':')?;
                        let token = p.bare()?;
                        let cell = as_cell_id(&token.to_ascii_lowercase()).ok_or_else(|| {
                            format!("`binders` maps binders to cell-id literals, got `{token}`")
                        })?;
                        if p.binders.insert(binder.clone(), cell).is_some() {
                            return Err(format!(
                                "binder `{binder}` bound twice (`binders` also must \
                                 precede any use of them)"
                            ));
                        }
                        Ok(())
                    })?;
                    Ok(())
                }
                "cells" => {
                    if cells_seen {
                        return Err("two `cells` tables".to_string());
                    }
                    cells_seen = true;
                    p.eat('{')?;
                    p.separated('}', |p| {
                        let token = p.token()?;
                        let cell = p.resolve(token);
                        // Stating a cell twice — by any spelling —
                        // fails rather than clobbering.
                        if !stated.insert(cell) {
                            return Err("a cell stated twice".to_string());
                        }
                        p.eat(':')?;
                        cells.set_value(cell, p.value()?);
                        Ok(())
                    })?;
                    Ok(())
                }
                "root" => {
                    if root.replace(p.value()?).is_some() {
                        return Err("two `root` fields".to_string());
                    }
                    Ok(())
                }
                other => Err(format!("unknown file field `{other}`")),
            }
        })?;
        Ok(Document { root, cells })
    }
}

/// The canonical printer — deterministic from (document, binders).
#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn print(doc: &Document, binders: &Binders) -> String {
    // Loaded binders survive for cell identities the document still mentions.
    // Every other identity receives a deterministic binder: its
    // simple name when available, otherwise its short id suffix.
    let mentioned = mentioned_cell_ids(doc);
    let mut spell: BTreeMap<CellId, String> = BTreeMap::new();
    for (binder, cell) in binders {
        if mentioned.contains(cell) {
            spell.entry(*cell).or_insert_with(|| binder.clone());
        }
    }
    let mut taken: HashSet<String> = spell.values().cloned().collect();
    let mut named: Vec<(&str, CellId)> = doc
        .cells
        .iter()
        .filter_map(|(cell, value)| name::read(value).map(|name| (name, *cell)))
        .collect();
    named.sort();
    for (name, cell) in &named {
        if spell.contains_key(cell) {
            continue;
        }
        let candidate = unique_binder(derive_binder(name), &mut taken);
        spell.insert(*cell, candidate);
    }
    let mut remaining: Vec<CellId> = mentioned
        .iter()
        .copied()
        .filter(|cell| !spell.contains_key(cell))
        .collect();
    remaining.sort();
    for cell in remaining {
        let hex = cell.simple().to_string();
        let candidate = unique_binder(format!("_{}", &hex[hex.len() - 5..]), &mut taken);
        spell.insert(cell, candidate);
    }

    let mut out = String::new();
    out.push_str("{\n");
    if !spell.is_empty() {
        out.push_str("  \"binders\": {\n");
        let mut table: Vec<(&String, &CellId)> =
            spell.iter().map(|(cell, binder)| (binder, cell)).collect();
        table.sort();
        for (binder, cell) in table {
            let _ = writeln!(out, "    {}: {},", quoted(binder), cell.simple());
        }
        out.push_str("  },\n");
    }
    let mut entries: Vec<(CellId, &Value)> = doc
        .cells
        .iter()
        .map(|(cell, value)| (*cell, value))
        .collect();
    entries.sort_by(|a, b| match (name::read(a.1), name::read(b.1)) {
        (Some(x), Some(y)) => x.cmp(y).then(a.0.cmp(&b.0)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });
    if !entries.is_empty() {
        out.push_str("  \"cells\": {\n");
        for (cell, value) in entries {
            out.push_str("    ");
            out.push_str(&identity(&spell, cell));
            out.push_str(": ");
            print_value(&mut out, value, &spell, 2);
            out.push_str(",\n");
        }
        out.push_str("  },\n");
    }
    if let Some(root) = &doc.root {
        out.push_str("  \"root\": ");
        print_value(&mut out, root, &spell, 1);
        out.push_str(",\n");
    }
    out.push_str("}\n");
    out
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn identity(spell: &BTreeMap<CellId, String>, cell: CellId) -> String {
    match spell.get(&cell) {
        Some(binder) => binder.clone(),
        None => cell.simple().to_string(),
    }
}

/// Every cell identity the document mentions: cell entries, links in values,
/// cells used as labels.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn mentioned_cell_ids(doc: &Document) -> HashSet<CellId> {
    let mut ids: HashSet<CellId> = doc.cells.cells().copied().collect();
    fn walk(value: &Value, ids: &mut HashSet<CellId>) {
        if plain_text(value).is_none() {
            match value {
                Value::Cell(cell) => {
                    ids.insert(*cell);
                }
                Value::Blob(_) => {}
                Value::List(elements) => {
                    for element in elements.values() {
                        walk(element, ids);
                    }
                }
                Value::Record(fields) => {
                    for (label, field) in fields {
                        ids.insert(*label);
                        walk(field, ids);
                    }
                }
            }
        }
    }
    for cell in doc.cells.cells().copied().collect::<Vec<_>>() {
        if let Some(value) = doc.cells.value(cell) {
            walk(value, &mut ids);
        }
    }
    if let Some(root) = &doc.root {
        walk(root, &mut ids);
    }
    ids
}

/// The notation's own string spelling — exactly the four escapes the
/// parser knows, everything else raw.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A name sanitized to the binder token grammar.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn derive_binder(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if !out
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
    {
        out.insert(0, '_');
    }
    if as_cell_id(&out.to_ascii_lowercase()).is_some() {
        out.insert(0, '_');
    }
    out
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn unique_binder(base: String, taken: &mut HashSet<String>) -> String {
    if taken.insert(base.clone()) {
        base
    } else {
        (2..)
            .map(|suffix| format!("{base}_{suffix}"))
            .find(|candidate| taken.insert(candidate.clone()))
            .expect("the integer suffix space is unbounded")
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn print_value(out: &mut String, value: &Value, spell: &BTreeMap<CellId, String>, level: usize) {
    let indent = "  ".repeat(level + 1);
    let closing = "  ".repeat(level);
    match plain_text(value) {
        Some(text) => {
            let _ = write!(out, "{}", quoted(text));
        }
        None => match value {
            Value::Blob(bytes) => {
                out.push_str("0x");
                for byte in bytes {
                    let _ = write!(out, "{byte:02x}");
                }
            }
            Value::Cell(cell) => out.push_str(&identity(spell, *cell)),
            Value::List(elements) if elements.is_empty() => out.push_str("[]"),
            Value::List(elements) => {
                out.push_str("[\n");
                for element in elements.values() {
                    out.push_str(&indent);
                    print_value(out, element, spell, level + 1);
                    out.push_str(",\n");
                }
                out.push_str(&closing);
                out.push(']');
            }
            Value::Record(fields) if fields.is_empty() => out.push_str("{}"),
            Value::Record(fields) => {
                out.push_str("{\n");
                for (label, field) in fields {
                    out.push_str(&indent);
                    out.push_str(&identity(spell, *label));
                    out.push_str(": ");
                    print_value(out, field, spell, level + 1);
                    out.push_str(",\n");
                }
                out.push_str(&closing);
                out.push('}');
            }
        },
    }
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn plain_text(value: &Value) -> Option<&str> {
    let text = text::read(value)?;
    value
        .as_record()?
        .keys()
        .all(|label| *label == text::vocabulary::UTF8)
        .then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(text: &str) -> (Document, Binders) {
        parse(text).expect("parses")
    }

    #[test]
    fn a_file_round_trips_canonically() {
        let text = r#"{
  "binders": {
    "color": 267770b27d00e45a18f730652ff90dcf,
    "name": 02e562654d6d0828d3a7559e6f75fffe,
    "payload": 6675eed11743101e3af8ec08c468da50,
    "shape": f7c17a58b92f3f1b6090142612b16595,
    "stroke": 0a6fba22bfd658dedb62920b6ea06699,
    "style": 0f950b2aa9ed44554b87ab8f88d82cea,
    "swatch": 0f3ae682742540de963d02d5f4b1a5a5,
    "tags": 98a45386e99286144421e94d8c444d9b,
  },
  "cells": {
    roof1: {name: "roof", stroke: "hairline", tags: ["draft", "gabled"]},
    9d2c1e10ab3440de963d02d5f4b1a5a5: {payload: 0x663399},
  },
  "root": {shape: roof1, style: 9d2c1e10ab3440de963d02d5f4b1a5a5, color: swatch},
}
"#;
        let (doc, binders) = parse_ok(text);
        assert_eq!(binders.len(), 9);
        let roof = binders["roof1"];
        assert_eq!(doc.cells.value(roof).and_then(name::read), Some("roof"));
        // The canonical print is a fixed point.
        let printed = print(&doc, &binders);
        let (again, binders_again) = parse_ok(&printed);
        assert_eq!(print(&again, &binders_again), printed);
        // The loaded binder survives the round trip.
        assert!(printed.contains("\"roof1\": "));
        assert!(printed.contains("\"swatch\": 0f3ae682742540de963d02d5f4b1a5a5"));
    }

    #[test]
    fn leniencies_normalize_and_minting_persists() {
        // Uppercase cell id, no trailing commas, a binder never declared:
        // all defined leniencies; saving canonicalizes.
        let text = r#"{"cells": {florp: "x"},
                       "root": {a: florp, b: 9D2C1E10AB3440DE963D02D5F4B1A5A5}}"#;
        let (doc, binders) = parse_ok(text);
        let florp = binders["florp"];
        assert_eq!(doc.cells.value(florp), Some(&crate::test_values::text("x")));
        let printed = print(&doc, &binders);
        assert!(printed.contains("\"florp\": "));
        assert!(printed.contains("9d2c1e10ab3440de963d02d5f4b1a5a5"));
        // A reference to a never-defined binder is a bare cell —
        // create-on-reference at the file layer.
        let (doc, binders) = parse_ok(r#"{"root": [ghost]}"#);
        let ghost = binders["ghost"];
        assert!(doc.cells.value(ghost).is_none());
        assert_eq!(doc.root, Some(Value::list([Value::from(ghost)])));
    }

    #[test]
    fn a_cell_stated_twice_fails_and_duplicate_names_coexist() {
        // The same cell twice, by the same spelling or different
        // ones, refuses rather than clobbering.
        assert!(parse(r#"{"cells": {x: "a", x: "b"}}"#).is_err());
        let aliased = r#"{
            "binders": {"x": 9d2c1e10ab3440de963d02d5f4b1a5a5},
            "cells": {
                x: "a",
                9d2c1e10ab3440de963d02d5f4b1a5a5: "b",
            },
        }"#;
        assert!(parse(aliased).is_err());
        // Duplicate simple-name facts are ordinary GID data.
        let (doc, _) = parse_ok(
            r#"{"binders": {
                "name": 02e562654d6d0828d3a7559e6f75fffe,
                "payload": 0a6fba22bfd658dedb62920b6ea06699
            }, "cells": {
                a: {name: "twin", payload: "a"},
                b: {name: "twin", payload: "b"},
            }}"#,
        );
        let twins = doc
            .cells
            .iter()
            .filter(|(_, value)| name::read(value) == Some("twin"))
            .count();
        assert_eq!(twins, 2);
    }

    #[test]
    fn derived_binders_come_from_simple_name_facts() {
        let mut cells = Cells::new();
        let cell = new_cell_id();
        cells.set_value(
            cell,
            name::record(
                "grap program",
                [(
                    crate::test_values::label("body"),
                    crate::test_values::text("body"),
                )],
            ),
        );
        let doc = Document {
            root: Some(Value::from(cell)),
            cells,
        };
        let printed = print(&doc, &Binders::new());
        assert!(printed.contains("\"grap_program\": "));
        assert!(printed.contains("    grap_program: {"));
        let (again, binders) = parse_ok(&printed);
        assert_eq!(
            again
                .cells
                .value(binders["grap_program"])
                .and_then(name::read),
            Some("grap program")
        );
        assert_eq!(print(&again, &binders), printed);
    }

    #[test]
    fn generated_binders_cover_every_identity_and_disambiguate() {
        let loaded = CellId::from_u128(0x0a6fba22bfd658dedb62920b6ea06699);
        let named_a = CellId::from_u128(0x6675eed11743101e3af8ec08c468da50);
        let named_b = CellId::from_u128(0x98a45386e99286144421e94d8c444d9b);
        let unnamed_a = CellId::from_u128(0x111111111111111111111111111abcde);
        let unnamed_b = CellId::from_u128(0x222222222222222222222222222abcde);
        let mut cells = Cells::new();
        cells.set_value(loaded, Value::from(vec![0]));
        cells.set_value(named_a, name::record("same", []));
        cells.set_value(named_b, name::record("same", []));
        cells.set_value(unnamed_a, Value::from(vec![1]));
        cells.set_value(unnamed_b, Value::from(vec![2]));
        let doc = Document { root: None, cells };
        let printed = print(&doc, &Binders::from([("same".to_string(), loaded)]));
        let (_, binders) = parse_ok(&printed);
        assert_eq!(binders["same"], loaded);
        assert_eq!(binders["same_2"], named_a);
        assert_eq!(binders["same_3"], named_b);
        assert_eq!(binders["_abcde"], unnamed_a);
        assert_eq!(binders["_abcde_2"], unnamed_b);
    }

    #[test]
    fn structural_damage_refuses() {
        assert!(parse("{").is_err());
        assert!(parse(r#"{"root": "unterminated}"#).is_err());
        assert!(parse(r#"{"root": {"label": "value"}}"#).is_err());
        assert!(parse(r#"{"root": {a: "x", a: "y"}}"#).is_err());
        assert!(parse(r#"{"root": 0x123}"#).is_err());
        assert!(parse(r#"{"cells": {x: }}"#).is_err());
        assert!(parse(r#"{"bogus": {}}"#).is_err());
        assert!(parse(r#"{"binders": {"x": notagid}}"#).is_err());
        assert!(parse(r#"{"root": 9-not-a-binder}"#).is_err());
    }

    #[test]
    fn text_escapes_and_blobs_round_trip() {
        let doc = Document {
            root: Some(Value::record([
                (
                    crate::test_values::label("s"),
                    crate::test_values::text("say \"hi\"\n\tdone\\"),
                ),
                (
                    crate::test_values::label("b"),
                    Value::from(vec![0x00_u8, 0xff]),
                ),
                (
                    crate::test_values::label("empty"),
                    Value::from(Vec::<u8>::new()),
                ),
            ])),
            cells: Cells::new(),
        };
        let printed = print(&doc, &Binders::new());
        let (again, _) = parse_ok(&printed);
        assert_eq!(again.root, doc.root);
    }

    #[test]
    fn the_empty_document_is_the_empty_record() {
        let (doc, binders) = parse_ok("{}");
        assert!(doc.root.is_none());
        assert!(doc.cells.cells().next().is_none());
        assert_eq!(print(&doc, &binders), "{\n}\n");
    }
}

#[cfg(test)]
mod checked_in_files {
    use super::*;

    fn root_reachable_cells(doc: &Document) -> HashSet<CellId> {
        fn enqueue(cell: CellId, reached: &mut HashSet<CellId>, pending: &mut Vec<CellId>) {
            if reached.insert(cell) {
                pending.push(cell);
            }
        }

        fn visit(value: &Value, reached: &mut HashSet<CellId>, pending: &mut Vec<CellId>) {
            match value {
                Value::Cell(cell) => enqueue(*cell, reached, pending),
                Value::Blob(_) => {}
                Value::List(elements) => {
                    for value in elements.values() {
                        visit(value, reached, pending);
                    }
                }
                Value::Record(fields) => {
                    for (label, value) in fields {
                        enqueue(*label, reached, pending);
                        visit(value, reached, pending);
                    }
                }
            }
        }

        let mut reached = HashSet::new();
        let mut pending = Vec::new();
        if let Some(root) = &doc.root {
            visit(root, &mut reached, &mut pending);
        }
        while let Some(cell) = pending.pop() {
            if let Some(value) = doc.cells.value(cell) {
                visit(value, &mut reached, &mut pending);
            }
        }
        reached
    }

    /// The checked-in sample is canonical: it parses, and printing
    /// it back is the identity — the printer's golden fixture.
    #[test]
    fn the_sample_file_is_a_fixed_point() {
        let text = include_str!("../../examples/sample.gid");
        let (doc, binders) = parse(text).expect("the sample parses");
        assert!(doc.root.is_some());
        assert_eq!(print(&doc, &binders), text);
    }

    #[test]
    fn every_sample_cell_definition_is_root_reachable() {
        let (doc, _) =
            parse(include_str!("../../examples/sample.gid")).expect("the sample parses");
        let reached = root_reachable_cells(&doc);
        let mut orphans: Vec<_> = doc
            .cells
            .cells()
            .filter(|cell| !reached.contains(cell))
            .copied()
            .collect();
        orphans.sort();
        assert!(orphans.is_empty(), "orphan cell definitions: {orphans:?}");
    }

    #[test]
    fn the_grap_demo_is_a_fixed_point() {
        let text = include_str!("../../examples/grap-demo.gid");
        let (doc, binders) = parse(text).expect("the Grap demo parses");
        assert_eq!(print(&doc, &binders), text);
    }

    #[test]
    fn the_iop_tree_demo_is_a_fixed_point_without_orphans() {
        let text = include_str!("../../examples/iop-tree.gid");
        let (doc, binders) = parse(text).expect("the IoP tree demo parses");
        assert_eq!(print(&doc, &binders), text);
        let reached = root_reachable_cells(&doc);
        assert!(doc.cells.cells().all(|cell| reached.contains(cell)));
    }
}
