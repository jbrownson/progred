//! The raw projection: any gid document rendered as entity blocks of
//! edge rows, with no schema required. The happy path is smooth (names
//! inline, cons chains as `[a, b, c]`) and everything else falls
//! through — SID labels render plain, GUID labels get circular
//! identicons, unparsable ids render as what they are.

use crate::conventions::{EMPTY, HEAD, NAME, TAIL};
use crate::identicon::{label_identicon, node_identicon};
use progred_graph::{Gid, Id, MutGid, NodeId, new_node_id};
use puri::draw::Canvas;
use puri::layout::{HAlign, Node, col, pad, row};
use puri::text::{TextCtx, TextStyle, text};
use vello::kurbo::Insets;
use vello::peniko::{Brush, Color};

pub struct RawStyles {
    pub name: TextStyle,
    pub label: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    pub scale: f64,
}

impl RawStyles {
    pub fn new(scale: f64) -> Self {
        let style = |size: f32, color: [f32; 4], weight: Option<f32>| TextStyle {
            size,
            brush: Brush::from(Color::new(color)),
            weight,
        };
        Self {
            name: style(15.0, [0.95, 0.95, 0.97, 1.0], Some(600.0)),
            label: style(14.0, [0.62, 0.66, 0.74, 1.0], None),
            string: style(14.0, [0.62, 0.83, 0.63, 1.0], None),
            number: style(14.0, [0.90, 0.72, 0.48, 1.0], None),
            dim: style(13.0, [0.48, 0.51, 0.58, 1.0], None),
            scale,
        }
    }
}

/// A small document exercising the model's range: named nodes, SID and
/// GUID labels, a cons list, an unnamed scratch node, and a value from
/// an unknown space.
pub fn sample_document() -> MutGid {
    let mut gid = MutGid::new();
    let name = Id::from(NAME);

    let origin = new_node_id();
    gid.set(origin, name.clone(), Id::from("origin"));
    gid.set(origin, Id::from("x"), Id::from(0.0));
    gid.set(origin, Id::from("y"), Id::from(0.0));

    let corner = new_node_id();
    gid.set(corner, name.clone(), Id::from("corner"));
    gid.set(corner, Id::from("x"), Id::from(4.0));
    gid.set(corner, Id::from("y"), Id::from(2.5));

    let cell2 = new_node_id();
    gid.set(cell2, Id::from(HEAD), Id::from(corner));
    gid.set(cell2, Id::from(TAIL), Id::from(EMPTY));
    let cell1 = new_node_id();
    gid.set(cell1, Id::from(HEAD), Id::from(origin));
    gid.set(cell1, Id::from(TAIL), Id::from(cell2));

    let stroke_width = new_node_id();
    gid.set(stroke_width, name.clone(), Id::from("stroke-width"));

    let polygon = new_node_id();
    gid.set(polygon, name.clone(), Id::from("polygon"));
    gid.set(polygon, Id::from("points"), Id::from(cell1));
    gid.set(polygon, Id::from(stroke_width), Id::from(1.5));

    let scratch = new_node_id();
    gid.set(scratch, Id::from("color"), Id::from("rebeccapurple"));
    gid.set(
        scratch,
        Id::from("mystery"),
        Id::in_space(new_node_id(), vec![0xde, 0xad, 0xbe, 0xef]),
    );

    gid
}

pub fn project<P: Canvas>(
    gid: &MutGid,
    tcx: &mut TextCtx,
    styles: &RawStyles,
) -> Node<P> {
    let mut entities: Vec<NodeId> = gid
        .entities()
        .copied()
        .filter(|entity| {
            // Cons cells render inline as list syntax, not as blocks.
            gid.get(&Id::from(*entity), &Id::from(HEAD)).is_none()
        })
        .collect();
    entities.sort();
    let blocks = entities
        .iter()
        .map(|entity| entity_block(gid, *entity, tcx, styles))
        .collect();
    col(HAlign::Start, 0, 14.0 * styles.scale, blocks)
}

fn entity_block<P: Canvas>(
    gid: &MutGid,
    entity: NodeId,
    tcx: &mut TextCtx,
    styles: &RawStyles,
) -> Node<P> {
    let id = Id::from(entity);
    let gap = 6.0 * styles.scale;

    let mut header = vec![node_identicon(entity, 18.0 * styles.scale)];
    if let Some(name) = entity_name(gid, &id) {
        header.push(text(tcx, &name, &styles.name));
    }
    let header = row(gap, header);

    let mut edges: Vec<(Id, Id)> = gid
        .edges(&id)
        .map(|edges| edges.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    edges.sort();
    let rows = edges
        .into_iter()
        .filter(|(label, _)| label.as_node_id() != Some(NAME))
        .map(|(label, value)| {
            row(
                gap,
                vec![
                    label_view(gid, &label, tcx, styles),
                    value_view(gid, &value, tcx, styles, 0),
                ],
            )
        })
        .collect();

    col(
        HAlign::Start,
        0,
        4.0 * styles.scale,
        vec![
            header,
            pad(
                Insets::new(26.0 * styles.scale, 0.0, 0.0, 0.0),
                col(HAlign::Start, 0, 4.0 * styles.scale, rows),
            ),
        ],
    )
}

fn label_view<P: Canvas>(
    gid: &MutGid,
    label: &Id,
    tcx: &mut TextCtx,
    styles: &RawStyles,
) -> Node<P> {
    if let Some(s) = label.as_str() {
        text(tcx, s, &styles.label)
    } else if let Some(uuid) = label.as_node_id() {
        let mut parts = vec![label_identicon(uuid, 14.0 * styles.scale)];
        if let Some(name) = entity_name(gid, label) {
            parts.push(text(tcx, &name, &styles.label));
        }
        row(4.0 * styles.scale, parts)
    } else {
        unknown_view(label, tcx, styles)
    }
}

fn value_view<P: Canvas>(
    gid: &MutGid,
    value: &Id,
    tcx: &mut TextCtx,
    styles: &RawStyles,
    depth: usize,
) -> Node<P> {
    if let Some(s) = value.as_str() {
        text(tcx, &format!("\"{s}\""), &styles.string)
    } else if let Some(n) = value.as_number() {
        text(tcx, &n.to_string(), &styles.number)
    } else if let Some(uuid) = value.as_node_id() {
        match list_items(gid, value) {
            Some(items) if depth < 4 => list_view(gid, &items, tcx, styles, depth),
            _ => {
                let mut parts = vec![node_identicon(uuid, 14.0 * styles.scale)];
                if let Some(name) = entity_name(gid, value) {
                    parts.push(text(tcx, &name, &styles.name));
                }
                row(4.0 * styles.scale, parts)
            }
        }
    } else {
        unknown_view(value, tcx, styles)
    }
}

/// A value from a space the editor doesn't know: the space's identicon
/// plus the payload as hex. Raw UUIDs are never shown; identicons are
/// their visual form.
fn unknown_view<P: Canvas>(id: &Id, tcx: &mut TextCtx, styles: &RawStyles) -> Node<P> {
    let hex: String = id.payload().iter().map(|b| format!("{b:02x}")).collect();
    row(
        4.0 * styles.scale,
        vec![
            label_identicon(id.space(), 14.0 * styles.scale),
            text(tcx, &hex, &styles.dim),
        ],
    )
}

fn list_view<P: Canvas>(
    gid: &MutGid,
    items: &[Id],
    tcx: &mut TextCtx,
    styles: &RawStyles,
    depth: usize,
) -> Node<P> {
    let mut parts = vec![text(tcx, "[", &styles.label)];
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            parts.push(text(tcx, ", ", &styles.label));
        }
        parts.push(value_view(gid, item, tcx, styles, depth + 1));
    }
    parts.push(text(tcx, "]", &styles.label));
    row(0.0, parts)
}

/// Walks a cons chain; `Some` if the value is list-shaped (the empty
/// list or a chain of head/tail cells), cycle-guarded.
pub fn list_items(gid: &impl Gid, value: &Id) -> Option<Vec<Id>> {
    let head = Id::from(HEAD);
    let tail = Id::from(TAIL);
    let empty = Id::from(EMPTY);

    let mut items = Vec::new();
    let mut cursor = value.clone();
    while items.len() < 64 {
        if cursor == empty {
            return Some(items);
        }
        let item = gid.get(&cursor, &head)?;
        items.push(item.clone());
        cursor = gid.get(&cursor, &tail)?.clone();
    }
    None
}

fn entity_name(gid: &impl Gid, id: &Id) -> Option<String> {
    gid.get(id, &Id::from(NAME))?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_items_walks_chains_and_guards_cycles() {
        let mut gid = MutGid::new();
        let (a, b) = (new_node_id(), new_node_id());
        let (c1, c2) = (new_node_id(), new_node_id());
        gid.set(c1, Id::from(HEAD), Id::from(a));
        gid.set(c1, Id::from(TAIL), Id::from(c2));
        gid.set(c2, Id::from(HEAD), Id::from(b));
        gid.set(c2, Id::from(TAIL), Id::from(EMPTY));

        assert_eq!(
            list_items(&gid, &Id::from(c1)),
            Some(vec![Id::from(a), Id::from(b)])
        );
        assert_eq!(list_items(&gid, &Id::from(EMPTY)), Some(vec![]));
        assert_eq!(list_items(&gid, &Id::from(a)), None);

        // A cycle terminates as not-a-list instead of hanging.
        gid.set(c2, Id::from(TAIL), Id::from(c1));
        assert_eq!(list_items(&gid, &Id::from(c1)), None);
    }
}
