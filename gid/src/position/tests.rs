use super::*;

#[test]
fn cloned_positions_share_bytes_but_compare_by_content() {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let position = Position::from_bytes(vec![0, 42]).unwrap();
    let cloned = position.clone();
    let separate = Position::from_bytes(vec![0, 42]).unwrap();
    assert!(Arc::ptr_eq(&position.0, &cloned.0));
    assert!(!Arc::ptr_eq(&position.0, &separate.0));
    assert_eq!(position, separate);
    assert_eq!(position.cmp(&separate), std::cmp::Ordering::Equal);
    let hash = |position: &Position| {
        let mut state = DefaultHasher::new();
        position.hash(&mut state);
        state.finish()
    };
    assert_eq!(hash(&position), hash(&separate));
    drop(position);
    assert_eq!(cloned.as_bytes(), &[0, 42]);
    assert!(Position::from_bytes(vec![]).is_none());
    assert!(Position::from_bytes(vec![42, 0]).is_none());
}

#[test]
fn between_is_ordered_and_dense() {
    let first = between(None, None).unwrap();
    let mid = between(Some(&first), None).unwrap();
    assert!(first < mid);
    let low = between(None, Some(&first)).unwrap();
    assert!(low < first);
    let inner = between(Some(&low), Some(&first)).unwrap();
    assert!(low < inner && inner < first);
}

#[test]
fn appends_and_prepends_grow_about_a_byte_per_255_inserts() {
    let mut last = between(None, None).unwrap();
    for _ in 0..2000 {
        let next = between(Some(&last), None).unwrap();
        assert!(last < next);
        last = next;
    }
    assert!(last.0.len() <= 2000 / 255 + 2);

    let mut first = between(None, None).unwrap();
    for _ in 0..2000 {
        let previous = between(None, Some(&first)).unwrap();
        assert!(previous < first);
        first = previous;
    }
    assert!(first.0.len() <= 2000 / 255 + 2);
}

#[test]
fn appending_after_a_deep_position_steps_at_its_first_digit_with_room() {
    let deep = Position::from_bytes(vec![0x40, 0x12, 0x34]).unwrap();
    assert_eq!(between(Some(&deep), None).unwrap().0.as_ref(), [0x41]);
    let full = Position::from_bytes(vec![0xFF, 0xFF]).unwrap();
    assert_eq!(
        between(Some(&full), None).unwrap().0.as_ref(),
        [0xFF, 0xFF, 0x01]
    );
    let low = Position::from_bytes(vec![0x00, 0x01]).unwrap();
    let below = between(None, Some(&low)).unwrap();
    assert_eq!(below.0.as_ref(), [0x00, 0x00, 0xFF]);
    assert!(below < low);
}

#[test]
fn same_gap_bisection_grows_about_a_bit_per_insert() {
    let low = between(None, None).unwrap();
    let mut high = between(Some(&low), None).unwrap();
    for _ in 0..2000 {
        let mid = between(Some(&low), Some(&high)).unwrap();
        assert!(low < mid && mid < high);
        high = mid;
    }
    assert!(high.0.len() <= 2000 / 8 + 3);
}

#[test]
fn random_gap_insertions_keep_strict_order() {
    // Deterministic LCG so the test needs no clock or rand.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut step = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    let mut positions = vec![between(None, None).unwrap()];
    for _ in 0..3000 {
        let gap = step() % (positions.len() + 1);
        let low = gap.checked_sub(1).map(|i| &positions[i]);
        let high = positions.get(gap);
        let fresh = between(low, high).unwrap();
        assert!(low.is_none_or(|low| *low < fresh));
        assert!(high.is_none_or(|high| fresh < *high));
        positions.insert(gap, fresh);
    }
    assert!(positions.is_sorted());
}

#[test]
fn spread_mints_even_ordered_positions() {
    for n in [0, 1, 2, 3, 127, 128, 1000] {
        let positions = spread(n);
        assert_eq!(positions.len(), n);
        // Every gap admits a between — strict order included,
        // since between demands it.
        assert!(between(None, positions.first()).is_some() || n == 0);
        assert!(between(positions.last(), None).is_some() || n == 0);
        for pair in positions.windows(2) {
            assert!(between(Some(&pair[0]), Some(&pair[1])).is_some());
        }
        // Even spreading, not append-chains: payloads stay near
        // log₂₅₆ n bytes instead of growing toward n/8.
        assert!(positions.iter().all(|p| p.0.len() <= 2));
    }
}

#[test]
fn between_declines_misordered_inputs() {
    let a = between(None, None).unwrap();
    assert!(between(Some(&a), Some(&a)).is_none());
    let b = between(Some(&a), None).unwrap();
    assert!(between(Some(&b), Some(&a)).is_none());
}
