//! The gid notation: the raw projection's grammar, made writable —
//! see docs/gid.md. The parser is lenient in the defined ways
//! only; the printer is the canon, and saving canonicalizes.

use crate::raw::Document;
use progred_graph::{Atom, CellId, Cells, Label, Value, new_cell_id};
use std::collections::BTreeMap;
use std::collections::HashSet;
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
    Gid(CellId),
    Binder(String),
}

fn as_gid(token: &str) -> Option<CellId> {
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

    /// A bare token as an identity: gid, or binder minting on first
    /// use — create-on-reference at the file layer.
    fn token(&mut self) -> Result<Token, String> {
        let token = self.bare()?;
        if let Some(gid) = as_gid(&token.to_ascii_lowercase()) {
            return Ok(Token::Gid(gid));
        }
        if !valid_binder(token) {
            return Err(format!("`{token}` is neither a gid nor a binder"));
        }
        Ok(Token::Binder(token.to_string()))
    }

    fn resolve(&mut self, token: Token) -> CellId {
        match token {
            Token::Gid(gid) => gid,
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
            Some('"') => Ok(Value::from(self.string()?)),
            Some('{') => {
                self.rest = &self.rest[1..];
                let fields = self.separated('}', |p| {
                    let label = p.label()?;
                    p.eat(':')?;
                    Ok((label, p.value()?))
                })?;
                let mut seen = HashSet::new();
                for (label, _) in &fields {
                    if !seen.insert(label.clone()) {
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

    fn label(&mut self) -> Result<Label, String> {
        match self.peek() {
            Some('"') => Ok(Label::from(self.string()?.as_str())),
            _ => {
                let token = self.token()?;
                Ok(Label::Cell(self.resolve(token)))
            }
        }
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
                        if as_gid(&binder.to_ascii_lowercase()).is_some() {
                            return Err(format!("`{binder}` spells a gid, not a binder"));
                        }
                        p.eat(':')?;
                        let token = p.bare()?;
                        let gid = as_gid(&token.to_ascii_lowercase()).ok_or_else(|| {
                            format!("`binders` maps binders to gid literals, got `{token}`")
                        })?;
                        if p.binders.insert(binder.clone(), gid).is_some() {
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
                        let gid = p.resolve(token);
                        // Stating a cell twice — by any spelling —
                        // fails rather than clobbering.
                        if !stated.insert(gid) {
                            return Err("a cell stated twice".to_string());
                        }
                        p.eat(':')?;
                        let entry = p.entry()?;
                        if let Some(name) = &entry.name {
                            cells.set_name(gid, name);
                        }
                        if let Some(value) = entry.value {
                            cells.set_value(gid, value);
                        }
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

    fn entry(&mut self) -> Result<Entry, String> {
        self.eat('{')?;
        let mut entry = Entry {
            name: None,
            value: None,
        };
        let fields = self.separated('}', |p| {
            let key = p.string()?;
            p.eat(':')?;
            Ok(match key.as_str() {
                "name" => EntryField::Name(p.string()?),
                "value" => EntryField::Value(p.value()?),
                other => return Err(format!("unknown entry field `{other}`")),
            })
        })?;
        for field in fields {
            match field {
                EntryField::Name(name) => {
                    if name.is_empty() {
                        return Err("an empty name is spelled by omission".to_string());
                    }
                    entry.name = Some(name);
                }
                EntryField::Value(value) => entry.value = Some(value),
            }
        }
        if entry.name.is_none() && entry.value.is_none() {
            return Err("an entry needs a name or a value".to_string());
        }
        Ok(entry)
    }
}

struct Entry {
    name: Option<String>,
    value: Option<Value>,
}

enum EntryField {
    Name(String),
    Value(Value),
}

/// The canonical printer — deterministic from (document, binders).
pub fn print(doc: &Document, binders: &Binders) -> String {
    // Binders survive for gids the document still mentions; cells
    // that gained a unique name earn a derived binder; the rest
    // spell as gid literals.
    let mentioned = mentioned_gids(doc);
    let mut spell: BTreeMap<CellId, String> = BTreeMap::new();
    for (binder, gid) in binders {
        if mentioned.contains(gid) {
            spell.entry(*gid).or_insert_with(|| binder.clone());
        }
    }
    let taken: HashSet<String> = spell.values().cloned().collect();
    let mut taken = taken;
    let mut named: Vec<(&str, CellId)> = doc
        .cells
        .iter()
        .filter_map(|(gid, entry)| entry.name().map(|name| (name, *gid)))
        .collect();
    named.sort();
    for (name, gid) in &named {
        if spell.contains_key(gid) {
            continue;
        }
        let candidate = derive_binder(name);
        if valid_binder(&candidate)
            && as_gid(&candidate.to_ascii_lowercase()).is_none()
            && taken.insert(candidate.clone())
        {
            spell.insert(*gid, candidate);
        }
    }

    let mut out = String::new();
    out.push_str("{\n");
    if !spell.is_empty() {
        out.push_str("  \"binders\": {\n");
        let mut table: Vec<(&String, &CellId)> =
            spell.iter().map(|(gid, binder)| (binder, gid)).collect();
        table.sort();
        for (binder, gid) in table {
            let _ = writeln!(out, "    {}: {},", quoted(binder), gid.simple());
        }
        out.push_str("  },\n");
    }
    let mut entries: Vec<(CellId, Option<&str>)> = doc
        .cells
        .iter()
        .map(|(gid, entry)| (*gid, entry.name()))
        .collect();
    entries.sort_by(|a, b| match (a.1, b.1) {
        (Some(x), Some(y)) => x.cmp(y).then(a.0.cmp(&b.0)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });
    if !entries.is_empty() {
        out.push_str("  \"cells\": {\n");
        for (gid, name) in entries {
            out.push_str("    ");
            out.push_str(&identity(&spell, gid));
            out.push_str(": {");
            if let Some(name) = name {
                let _ = write!(out, "\"name\": {}", quoted(name));
            }
            if let Some(value) = doc.cells.value(gid) {
                if name.is_some() {
                    out.push_str(", ");
                }
                out.push_str("\"value\": ");
                print_value(&mut out, value, &spell, 2);
            }
            out.push_str("},\n");
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

fn identity(spell: &BTreeMap<CellId, String>, gid: CellId) -> String {
    match spell.get(&gid) {
        Some(binder) => binder.clone(),
        None => gid.simple().to_string(),
    }
}

/// Every gid the document mentions: cell entries, links in values,
/// cells used as labels.
fn mentioned_gids(doc: &Document) -> HashSet<CellId> {
    let mut gids: HashSet<CellId> = doc.cells.cells().copied().collect();
    fn walk(value: &Value, gids: &mut HashSet<CellId>) {
        match value {
            Value::Atom(atom) => gids.extend(atom.as_cell()),
            Value::List(elements) => {
                for element in elements.values() {
                    walk(element, gids);
                }
            }
            Value::Record(fields) => {
                for (label, field) in fields {
                    gids.extend(label.as_cell());
                    walk(field, gids);
                }
            }
        }
    }
    for gid in doc.cells.cells().copied().collect::<Vec<_>>() {
        if let Some(value) = doc.cells.value(gid) {
            walk(value, &mut gids);
        }
    }
    if let Some(root) = &doc.root {
        walk(root, &mut gids);
    }
    gids
}

/// The notation's own string spelling — exactly the four escapes the
/// parser knows, everything else raw.
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
    out
}

fn print_value(out: &mut String, value: &Value, spell: &BTreeMap<CellId, String>, level: usize) {
    let indent = "  ".repeat(level + 1);
    let closing = "  ".repeat(level);
    match value {
        Value::Atom(Atom::String(s)) => {
            let _ = write!(out, "{}", quoted(s));
        }
        Value::Atom(Atom::Blob(bytes)) => {
            out.push_str("0x");
            for byte in bytes {
                let _ = write!(out, "{byte:02x}");
            }
        }
        Value::Atom(Atom::Cell(gid)) => out.push_str(&identity(spell, *gid)),
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
                match label {
                    Label::String(s) => {
                        let _ = write!(out, "{}", quoted(s));
                    }
                    Label::Cell(gid) => out.push_str(&identity(spell, *gid)),
                }
                out.push_str(": ");
                print_value(out, field, spell, level + 1);
                out.push_str(",\n");
            }
            out.push_str(&closing);
            out.push('}');
        }
    }
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
    "swatch": 0f3ae682742540de963d02d5f4b1a5a5,
  },
  "cells": {
    roof1: {"name": "roof", "value": {"stroke": "hairline", "tags": ["draft", "gabled"]}},
    9d2c1e10ab3440de963d02d5f4b1a5a5: {"value": {"payload": 0x663399}},
  },
  "root": {"shape": roof1, "style": 9d2c1e10ab3440de963d02d5f4b1a5a5, "color": swatch},
}
"#;
        let (doc, binders) = parse_ok(text);
        assert_eq!(binders.len(), 2);
        let roof = binders["roof1"];
        assert_eq!(doc.cells.name(roof), Some("roof"));
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
        // Uppercase gid, no trailing commas, a binder never declared:
        // all defined leniencies; saving canonicalizes.
        let text = r#"{"cells": {florp: {"value": "x"}},
                       "root": {"a": florp, "b": 9D2C1E10AB3440DE963D02D5F4B1A5A5}}"#;
        let (doc, binders) = parse_ok(text);
        let florp = binders["florp"];
        assert_eq!(doc.cells.value(florp), Some(&Value::from("x")));
        let printed = print(&doc, &binders);
        assert!(printed.contains("\"florp\": "));
        assert!(printed.contains("9d2c1e10ab3440de963d02d5f4b1a5a5"));
        // A reference to a never-defined binder is a bare cell —
        // create-on-reference at the file layer.
        let (doc, binders) = parse_ok(r#"{"root": [ghost]}"#);
        let ghost = binders["ghost"];
        assert!(doc.cells.entry(ghost).is_none());
        assert_eq!(doc.root, Some(Value::list([Value::from(ghost)])));
    }

    #[test]
    fn a_cell_stated_twice_fails_and_duplicate_names_coexist() {
        // The same cell twice, by the same spelling or different
        // ones, refuses rather than clobbering.
        assert!(parse(r#"{"cells": {x: {"value": "a"}, x: {"value": "b"}}}"#).is_err());
        let aliased = r#"{
            "binders": {"x": 9d2c1e10ab3440de963d02d5f4b1a5a5},
            "cells": {
                x: {"value": "a"},
                9d2c1e10ab3440de963d02d5f4b1a5a5: {"value": "b"},
            },
        }"#;
        assert!(parse(aliased).is_err());
        // Duplicate NAMES are the model's normal state.
        let (doc, _) = parse_ok(
            r#"{"cells": {
                a: {"name": "twin", "value": "a"},
                b: {"name": "twin", "value": "b"},
            }}"#,
        );
        let twins = doc
            .cells
            .iter()
            .filter(|(_, entry)| entry.name() == Some("twin"))
            .count();
        assert_eq!(twins, 2);
    }

    #[test]
    fn derived_binders_come_from_unique_names() {
        let mut cells = Cells::new();
        let gid = new_cell_id();
        cells.set_name(gid, "grap program");
        cells.set_value(gid, Value::from("body"));
        let doc = Document {
            root: Some(Value::from(gid)),
            cells,
        };
        let printed = print(&doc, &Binders::new());
        assert!(printed.contains("\"grap_program\": "));
        assert!(printed.contains("    grap_program: {\"name\": \"grap program\""));
        let (again, binders) = parse_ok(&printed);
        assert_eq!(
            again.cells.name(binders["grap_program"]),
            Some("grap program")
        );
        assert_eq!(print(&again, &binders), printed);
    }

    #[test]
    fn structural_damage_refuses() {
        assert!(parse("{").is_err());
        assert!(parse(r#"{"root": "unterminated}"#).is_err());
        assert!(parse(r#"{"root": {"a": "x", "a": "y"}}"#).is_err());
        assert!(parse(r#"{"root": 0x123}"#).is_err());
        assert!(parse(r#"{"cells": {x: {}}}"#).is_err());
        assert!(parse(r#"{"cells": {x: {"gid": y, "value": "v"}}}"#).is_err());
        assert!(parse(r#"{"cells": {x: {"name": "", "value": "v"}}}"#).is_err());
        assert!(parse(r#"{"bogus": {}}"#).is_err());
        assert!(parse(r#"{"binders": {"x": notagid}}"#).is_err());
        assert!(parse(r#"{"root": 9-not-a-binder}"#).is_err());
    }

    #[test]
    fn strings_escape_and_blobs_round_trip() {
        let doc = Document {
            root: Some(Value::record([
                (Label::from("s"), Value::from("say \"hi\"\n\tdone\\")),
                (Label::from("b"), Value::from(vec![0x00_u8, 0xff])),
                (Label::from("empty"), Value::from(Vec::<u8>::new())),
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
mod sample_file {
    use super::*;

    /// The checked-in sample is canonical: it parses, and printing
    /// it back is the identity — the printer's golden fixture.
    #[test]
    fn the_sample_file_is_a_fixed_point() {
        let text = include_str!("../../sample.gid");
        let (doc, binders) = parse(text).expect("the sample parses");
        assert!(doc.root.is_some());
        assert_eq!(print(&doc, &binders), text);
    }

    #[test]
    fn the_sample_contains_a_projectable_grap_computation() {
        let (doc, binders) = parse(include_str!("../../sample.gid")).expect("the sample parses");
        let roof = doc
            .root
            .as_ref()
            .and_then(Value::as_record)
            .and_then(|root| root.get(&Label::from("shape")))
            .and_then(Value::as_cell)
            .and_then(|roof| doc.cells.value(roof))
            .and_then(Value::as_record)
            .expect("roof record");
        let expression = roof
            .get(&Label::from("double pitch"))
            .expect("Grap expression");
        let foreign = crate::conventions::foreign_functions();
        assert_eq!(
            grap::evaluate(
                expression,
                |cell| doc.cells.value(cell).cloned(),
                &foreign,
                grap::DEFAULT_FUEL,
            )
            .result,
            Ok(grap_f64::value(5.0))
        );
        let profile = roof.get(&Label::from("profile")).expect("profile call");
        let evaluation = grap::evaluate(
            profile,
            |cell| doc.cells.value(cell).cloned(),
            &foreign,
            grap::DEFAULT_FUEL,
        );
        assert_eq!(evaluation.result, Ok(grap_geometry::value(40.0)));
        assert_eq!(
            evaluation.dependencies,
            [binders["double"], binders["pitch"]].into_iter().collect()
        );
    }
}
