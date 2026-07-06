//! A deterministic identicon sample sheet. Fixed seeds sweep hue x
//! silhouette, hue x chroma, and the chassis finish, then a random
//! gallery down to standard sizes, so palette and salience changes
//! are judged against the same identities every run. Unwired; give it
//! a view toggle in the shell when tuning.

use crate::identicon::node_identicon;
use crate::raw::RawStyles;
use progred_graph::NodeId;
use puri::draw::Canvas;
use puri::layout::{Extent, HAlign, Node, col, leaf, pad, row};
use puri::text::{TextCtx, TextStyle, text};
use std::iter::once;
use vello::kurbo::Insets;

const HUES: [&str; 8] = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink"];
const SILHOUETTES: [&str; 8] =
    ["square", "rounded", "disc", "annulus", "diamond", "shield", "hexflat", "hexpoint"];

fn mix(z: u64) -> u64 {
    let z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn bits(seed: u64) -> u128 {
    ((mix(seed) as u128) << 64) | mix(seed ^ 0xDEAD_BEEF) as u128
}

/// Deterministic sample id with bits `0..width` forced to `low`.
fn sample(seed: u64, low: u128, width: u32) -> NodeId {
    NodeId::from_u128((bits(seed) & !((1u128 << width) - 1)) | low)
}

/// Same id, forced chassis finish: grout, rim band, and bead size
/// (bits 54..60) all set to `step`.
fn sample_finish(seed: u64, step: u128) -> NodeId {
    NodeId::from_u128(
        (bits(seed) & !(0x3fu128 << 54)) | (step << 54) | (step << 56) | (step << 58),
    )
}

/// Columns grouped by family: low bits are family | variant << 2.
fn column_low(column: u64) -> u128 {
    ((column >> 1) | ((column & 1) << 2)) as u128
}

fn hspace<P>(width: f64) -> Node<P> {
    leaf(
        Extent {
            width,
            ascent: 0.0,
            descent: 0.0,
        },
        |_: &mut P, _| {},
    )
}

/// Pads `node` on the right up to `width`, so rows built from boxed
/// cells keep strict column pitch.
fn boxed<P: Canvas>(width: f64, node: Node<P>) -> Node<P> {
    let slack = (width - node.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, slack, 0.0), node)
}

pub fn sample_sheet<P: Canvas>(tcx: &mut TextCtx, styles: &RawStyles) -> Node<P> {
    let s = styles.scale;
    let icon = 32.0 * s;
    let pitch = 42.0 * s;
    let label_w = 56.0 * s;
    let header = TextStyle {
        size: 10.0,
        brush: styles.dim.brush.clone(),
        weight: None,
    };
    let caption = |tcx: &mut TextCtx, s: &str| text(tcx, s, &styles.dim);

    let family_header = row(
        0.0,
        once(hspace(label_w))
            .chain(SILHOUETTES.iter().map(|name| boxed(pitch, text(tcx, name, &header))))
            .collect(),
    );
    let hue_rows: Vec<Node<P>> = HUES
        .iter()
        .enumerate()
        .map(|(h, name)| {
            row(
                0.0,
                once(boxed(label_w, text(tcx, name, &styles.dim)))
                    .chain((0..8u64).map(|f| {
                        let id = sample(h as u64 * 8 + f, column_low(f) | ((h as u128) << 3), 6);
                        boxed(pitch, node_identicon(id, icon))
                    }))
                    .collect(),
            )
        })
        .collect();
    let hue_family = col(
        HAlign::Start,
        0,
        6.0 * s,
        once(family_header).chain(hue_rows).collect(),
    );

    let chroma_rows: Vec<Node<P>> = (0..8u64)
        .map(|h| {
            row(
                0.0,
                (0..4u64)
                    .map(|c| {
                        let id = sample(1000 + h * 4 + c, ((h as u128) << 3) | ((c as u128) << 6), 8);
                        boxed(pitch, node_identicon(id, icon))
                    })
                    .collect(),
            )
        })
        .collect();
    let finish = row(
        0.0,
        (0..2u64)
            .flat_map(|seed| {
                (0..4u128).map(move |step| (7 + seed, step)).collect::<Vec<_>>()
            })
            .map(|(seed, step)| boxed(pitch, node_identicon(sample_finish(seed, step), icon)))
            .collect(),
    );

    let gallery_row = |size: f64| {
        row(
            0.0,
            (0..14u64)
                .map(|k| boxed(46.0 * s, node_identicon(sample(2000 + k, 0, 0), size * s)))
                .collect(),
        )
    };

    col(
        HAlign::Start,
        0,
        12.0 * s,
        vec![
            caption(tcx, "identicon samples — press i to return"),
            row(
                24.0 * s,
                vec![
                    hue_family,
                    col(
                        HAlign::Start,
                        0,
                        6.0 * s,
                        once(hspace(4.0 * pitch))
                            .chain(chroma_rows)
                            .chain(once(finish))
                            .collect(),
                    ),
                ],
            ),
            caption(tcx, "gallery at 36 / 18 / 14"),
            gallery_row(36.0),
            gallery_row(18.0),
            gallery_row(14.0),
        ],
    )
}
