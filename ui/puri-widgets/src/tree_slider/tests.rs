use super::*;
fn leaves(key: u8, keys: &[u8]) -> Tree<u8> {
    Tree::group(key, keys.iter().copied().map(Tree::leaf))
}

#[test]
fn ticks_keep_hierarchy_height_and_unequal_group_lengths() {
    let tree = Tree::group(
        0,
        [
            leaves(10, &[11, 12]),
            Tree::group(20, [leaves(21, &[22, 23, 24]), leaves(25, &[26])]),
        ],
    );
    let all = Selection::new(&tree, &[]);
    let ticks = |selection: &Selection<u8>| {
        selection
            .ticks()
            .into_iter()
            .map(|level| (level.values, level.height, level.width))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ticks(&all),
        [
            (
                vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
                24.0 / 2.25,
                3.0 / 2.25
            ),
            (vec![0.0, 2.0, 5.0, 6.0], 16.0, 2.0),
            (vec![0.0, 2.0, 6.0], 24.0, 3.0),
        ]
    );
    assert_eq!(
        ticks(&all.select(&tree, 0, 2..5)),
        [
            (vec![2.0, 3.0, 4.0, 5.0], 24.0 / 2.25, 3.0 / 2.25),
            (vec![2.0, 5.0], 16.0, 2.0),
            (vec![2.0], 24.0, 3.0),
        ]
    );
    assert!(Selection::new(&leaves(0, &[]), &[]).ticks().is_empty());
}

#[test]
fn coarser_changes_reset_finer_selections_instead_of_reviving_them() {
    let tree = Tree::group(0, [leaves(10, &[11, 12]), leaves(20, &[21, 22])]);
    let all = Selection::new(&tree, &[]);
    let first = all.select(&tree, 1, 0..1);
    assert_eq!(first.leaves, 0..2);
    assert_eq!(first.intent[0], Intent::All);
    assert_eq!(first.select_all(&tree, 1).leaves, 0..4);

    let chosen = all.select(&tree, 0, 1..3);
    assert_eq!(chosen.leaves, 1..3);
    let first = chosen.select(&tree, 1, 0..1);
    assert_eq!(first.leaves, 0..2);
    assert_eq!(first.intent[0], Intent::All);
    assert_eq!(
        chosen.select_all(&tree, 1).intent,
        [Intent::All, Intent::All]
    );
    let second = first.select(&tree, 1, 1..2);
    assert_eq!(second.leaves, 2..4);
    assert_eq!(second.intent[0], Intent::All);
    assert_eq!(second.select(&tree, 1, 0..1).leaves, 0..2);
    assert_eq!(second.select_all(&tree, 1).leaves, 0..4);
}

#[test]
fn selecting_the_current_item_preserves_finer_intent() {
    let tree = Tree::group(0, [leaves(10, &[11, 12]), leaves(20, &[21, 22])]);
    let chosen = Selection::new(&tree, &[]).select(&tree, 0, 1..3);
    let current = chosen.current_item(1, 1.5);
    assert_eq!(current, Some(0));
    let focused = chosen.select_with_current(&tree, 1, 0..1, current);
    assert_eq!(focused.leaves, 1..2);
    assert_eq!(focused.intent[0], chosen.intent[0]);
    assert_eq!(
        focused.select_with_current(&tree, 1, 0..1, current).intent,
        focused.intent
    );
    for reset in [
        focused.select_with_current(&tree, 1, 1..2, current),
        focused.select_with_current(&tree, 1, 0..2, current),
        focused.select(&tree, 1, 0..1),
        focused.select_all(&tree, 1),
    ] {
        assert_eq!(reset.intent[0], Intent::All);
    }
    let end = focused.current_item(1, 2.0);
    assert_eq!(end, Some(0));
    assert_eq!(
        focused.select_with_current(&tree, 1, 0..1, end).leaves,
        1..2
    );
    let dragged = focused.select_with_current(&tree, 1, 0..2, current);
    assert_eq!(
        dragged.select_with_current(&tree, 1, 0..1, current).intent[0],
        Intent::All,
        "returning within a drag does not resurrect the old finer filter"
    );
}

#[test]
fn intermediate_changes_clear_only_finer_levels_even_when_the_range_is_unchanged() {
    let tree = Tree::group(
        0,
        [
            Tree::group(10, [leaves(11, &[12, 13]), leaves(14, &[15, 16])]),
            Tree::group(20, [leaves(21, &[22, 23]), leaves(24, &[25, 26])]),
        ],
    );
    let selected = Selection::new(&tree, &[])
        .select(&tree, 2, 1..2)
        .select(&tree, 1, 0..1)
        .select(&tree, 0, 1..2);
    assert_eq!(selected.leaves, 5..6);
    for adjusted in [
        selected.select(&tree, 1, 0..1),
        selected.select_all(&tree, 1),
    ] {
        assert_eq!(adjusted.intent[0], Intent::All);
        assert_eq!(adjusted.intent[2], selected.intent[2]);
    }
    assert_eq!(selected.select(&tree, 1, 0..1).leaves, 4..6);
    assert_eq!(selected.select_all(&tree, 1).leaves, 4..8);
    assert_eq!(
        selected.select(&tree, 0, 0..1).intent[1..],
        selected.intent[1..]
    );
}

#[test]
fn inserted_and_deleted_endpoints_use_ordered_bounds_not_ordinals() {
    let tree = leaves(0, &[10, 30, 50, 70]);
    let selected = Selection::new(&tree, &[]).select(&tree, 0, 1..3);
    let edited = leaves(0, &[5, 10, 30, 40, 50, 60, 70]);
    assert_eq!(Selection::new(&edited, &selected.intent).leaves, 2..5);
    let deleted_ends = leaves(0, &[5, 10, 40, 60, 70]);
    assert_eq!(Selection::new(&deleted_ends, &selected.intent).leaves, 2..3);
    let disjoint = leaves(0, &[80, 90]);
    let visible = Selection::new(&disjoint, &selected.intent);
    assert_eq!(visible.leaves, 0..2);
    assert_eq!(visible.intent, selected.intent);
    assert_eq!(Selection::new(&tree, &visible.intent).leaves, 1..3);
}

#[test]
fn manual_full_span_is_bounded_but_all_includes_new_items() {
    let tree = leaves(0, &[10, 20]);
    let all = Selection::new(&tree, &[]);
    let explicit = all.select(&tree, 0, 0..2);
    let larger = leaves(0, &[5, 10, 15, 20, 30]);
    assert_eq!(Selection::new(&larger, &explicit.intent).leaves, 1..4);
    assert_eq!(Selection::new(&larger, &all.intent).leaves, 0..5);
    assert_eq!(
        Selection::new(&larger, &explicit.select_all(&tree, 0).intent).leaves,
        0..5
    );
}

#[test]
fn coarser_changes_clear_hidden_finer_levels_and_keep_fine_end_alignment() {
    let tree = Tree::group(
        0,
        [
            leaves(10, &[11, 12]),
            Tree::group(20, [Tree::group(21, [leaves(22, &[23, 24])])]),
            leaves(30, &[]),
        ],
    );
    let selected = Selection::new(&tree, &[]).select(&tree, 1, 1..2);
    assert_eq!(
        selected
            .rows
            .iter()
            .map(|r| (r.level, r.slider.count))
            .collect::<Vec<_>>(),
        [(3, 2), (2, 2), (1, 2), (0, 2)]
    );
    let shallow = selected.select(&tree, 3, 0..1);
    assert_eq!(
        shallow.rows.iter().map(|r| r.level).collect::<Vec<_>>(),
        [3, 0]
    );
    assert_eq!(shallow.intent[..3], [Intent::All, Intent::All, Intent::All]);
    assert_eq!(shallow.select_all(&tree, 3).leaves, 0..4);
    assert!(Selection::new(&leaves(0, &[]), &[]).slider(0.0).is_none());
}

#[test]
fn current_items_follow_playback_through_unequal_and_hidden_levels() {
    let tree = Tree::group(
        0,
        [
            leaves(10, &[11, 12]),
            Tree::group(20, [Tree::group(21, [leaves(22, &[23, 24, 25])])]),
            Tree::leaf(30),
        ],
    );
    let current = |selection: &Selection<u8>, position| {
        selection
            .rows
            .iter()
            .map(|row| {
                (
                    row.level,
                    selection.current_item(row.level, position).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    let all = Selection::new(&tree, &[]);
    assert_eq!(current(&all, 0.5), [(3, 0), (2, 0), (1, 0), (0, 0)]);
    assert_eq!(current(&all, 2.0), [(3, 1), (2, 1), (1, 1), (0, 2)]);
    assert_eq!(current(&all, 5.0), [(3, 2), (2, 2), (1, 2), (0, 5)]);
    assert_eq!(current(&all, 6.0), current(&all, 5.5));

    let deep = all.select(&tree, 3, 1..2);
    assert_eq!(current(&deep, 4.5), [(3, 1), (2, 0), (1, 0), (0, 2)]);
    assert_eq!(current(&deep, 5.0), current(&deep, 4.5));
    let shorter = deep.select(&tree, 0, 0..2);
    assert_eq!(current(&shorter, 4.0), [(3, 1), (2, 0), (1, 0), (0, 1)]);

    let shallow = all.select(&tree, 3, 0..1);
    assert_eq!(current(&shallow, 2.0), [(3, 0), (0, 1)]);
    assert_eq!(shallow.current_item(1, 0.5), None);
    assert_eq!(
        Selection::new(&leaves(0, &[]), &[]).current_item(0, 0.0),
        None
    );
}

#[test]
fn cursor_tracks_its_leaf_and_fraction_across_insertions() {
    let tree = leaves(0, &[10, 20]);
    let cursor = tree.cursor(1.25).unwrap();
    assert_eq!(
        cursor,
        Cursor {
            item: 20,
            fraction: 0.25
        }
    );
    assert_eq!(leaves(0, &[5, 10, 20]).position(&cursor), Some(2.25));
    assert_eq!(
        tree.cursor(2.0),
        Some(Cursor {
            item: 20,
            fraction: 1.0
        })
    );
    assert_eq!(leaves(0, &[10]).position(&cursor), None);
    let selection = Selection::new(&tree, &[]).select(&tree, 0, 0..1);
    assert_eq!(selection.position(1.25), 0.0);
    assert_eq!(
        selection.cursor(&tree, 1.0),
        Some(Cursor {
            item: 10,
            fraction: 1.0
        })
    );
}
