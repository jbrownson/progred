use super::*;

const WORDS: [&str; 4] = ["Alpha", "Beta", "alphabet", "Gamma"];

fn names(needle: &str) -> Vec<&'static str> {
    rank(WORDS.to_vec(), |w| w, needle)
        .into_iter()
        .map(|ranked| ranked.item)
        .collect()
}

#[test]
fn empty_needle_accepts_everything_in_order_with_no_spans() {
    let ranked = rank(WORDS.to_vec(), |w| w, "");
    assert_eq!(
        ranked.iter().map(|r| r.item).collect::<Vec<_>>(),
        WORDS.to_vec()
    );
    assert!(ranked[0].matches.is_empty());
}

#[test]
fn tiers_rank_prefix_over_substring_over_fuzzy() {
    // Exact prefix beats the case-insensitive one.
    assert_eq!(names("Al"), vec!["Alpha", "alphabet"]);
    // Exact prefix of "alphabet" beats Alpha's case-insensitive.
    assert_eq!(names("alp"), vec!["alphabet", "Alpha"]);
    // Substrings sort by fraction matched.
    assert_eq!(names("ph"), vec!["Alpha", "alphabet"]);
    // Exact fuzzy beats case-insensitive fuzzy; within the
    // insensitive tier, Gamma's fraction (2/5) beats alphabet's.
    assert_eq!(names("Aa"), vec!["Alpha", "Gamma", "alphabet"]);
    // No subsequence anywhere: rejected entirely.
    assert_eq!(names("xyz"), Vec::<&str>::new());
}

#[test]
fn match_spans_are_byte_offsets_on_the_original() {
    let ranked = rank(vec!["Alpha"], |w| w, "ph");
    assert_eq!(ranked[0].matches, vec![2..4]);

    // Multibyte haystacks keep spans byte-correct.
    let ranked = rank(vec!["état"], |w| w, "éa");
    assert_eq!(ranked[0].matches, vec![0..2, 3..4]);

    // Case-insensitive matching never shifts offsets.
    let ranked = rank(vec!["État"], |w| w, "ét");
    assert_eq!(ranked[0].matches, vec![0..3]);
}

#[test]
fn aliases_rank_once_without_highlighting_unmatched_display_text() {
    let offers = [
        ("new list", &["[", "list"][..]),
        ("list of values", &[][..]),
    ];
    let search =
        |query| rank_with_aliases(offers.to_vec(), |offer| offer.0, |offer| offer.1, query);
    let alias = search("list");
    assert_eq!(alias.len(), 2);
    assert_eq!(alias[0].item.0, "new list");
    assert!(alias[0].matches.is_empty());
    assert_eq!(alias[1].matches, vec![0..4]);
    let bracket = search("[");
    assert_eq!(bracket.len(), 1);
    assert_eq!(bracket[0].item.0, "new list");
    assert!(bracket[0].matches.is_empty());
    assert_eq!(search("new")[0].matches, vec![0..3]);
    let empty = search("");
    assert_eq!(
        empty.iter().map(|ranked| ranked.item).collect::<Vec<_>>(),
        offers
    );
    assert!(empty.iter().all(|ranked| ranked.matches.is_empty()));
}
