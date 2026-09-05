//! Compositional completion filtering, carried from the TypeScript
//! prototype (archived at `archive/pre-root-promotion`): tiers
//! chain over the previous tier's rejects, so every item lands in its
//! best tier — exact prefix, exact substring, their case-insensitive
//! forms, then fuzzy subsequence — with each tier sorted by the
//! fraction of the haystack matched, and match spans (byte offsets)
//! driving highlight rendering. Unlike the TypeScript version,
//! case-insensitive tiers compare per character on the original
//! string, so spans stay byte-correct under case folding.

use std::ops::Range;

pub struct Ranked<A> {
    pub item: A,
    pub matches: Vec<Range<usize>>,
    tier: usize,
}

impl<A> Ranked<A> {
    /// Whether the item was accepted by a fuzzy-subsequence tier
    /// rather than a prefix/substring one — a weak signal callers may
    /// rank differently.
    pub fn fuzzy(&self) -> bool {
        self.tier >= 4
    }
}

type CharEq = fn(char, char) -> bool;

fn exact(a: char, b: char) -> bool {
    a == b
}

fn case_insensitive(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn prefix(needle: &str, haystack: &str, eq: CharEq) -> Option<Vec<Range<usize>>> {
    let mut len = 0;
    let mut haystack_chars = haystack.chars();
    for n in needle.chars() {
        let h = haystack_chars.next()?;
        if !eq(n, h) {
            return None;
        }
        len += h.len_utf8();
    }
    Some(vec![0..len])
}

fn substring(needle: &str, haystack: &str, eq: CharEq) -> Option<Vec<Range<usize>>> {
    haystack.char_indices().find_map(|(start, _)| {
        let mut len = 0;
        let mut haystack_chars = haystack[start..].chars();
        for n in needle.chars() {
            let h = haystack_chars.next()?;
            if !eq(n, h) {
                return None;
            }
            len += h.len_utf8();
        }
        Some(vec![start..start + len])
    })
}

fn fuzzy(needle: &str, haystack: &str, eq: CharEq) -> Option<Vec<Range<usize>>> {
    let mut matches = Vec::new();
    let mut haystack_chars = haystack.char_indices();
    for n in needle.chars() {
        let (start, h) = haystack_chars.find(|(_, h)| eq(n, *h))?;
        matches.push(start..start + h.len_utf8());
    }
    Some(matches)
}

/// Ranks `items` against `needle`: the accepted, in tier order. An
/// empty needle accepts everything in the given order with NO match
/// spans — a span means "these characters matched your query", and
/// an empty query matched none (the popup bolds spans; nothing typed,
/// nothing bold).
pub fn rank<A>(items: Vec<A>, key: impl Fn(&A) -> &str, needle: &str) -> Vec<Ranked<A>> {
    if needle.is_empty() {
        return items
            .into_iter()
            .map(|item| Ranked {
                item,
                matches: Vec::new(),
                tier: 0,
            })
            .collect();
    }
    let tiers: [(fn(&str, &str, CharEq) -> Option<Vec<Range<usize>>>, CharEq); 6] = [
        (prefix, exact),
        (substring, exact),
        (prefix, case_insensitive),
        (substring, case_insensitive),
        (fuzzy, exact),
        (fuzzy, case_insensitive),
    ];
    let mut remaining = items;
    let mut ranked = Vec::new();
    for (tier, (matcher, eq)) in tiers.into_iter().enumerate() {
        let mut accepted = Vec::new();
        remaining = remaining
            .into_iter()
            .filter_map(|item| match matcher(needle, key(&item), eq) {
                Some(matches) => {
                    accepted.push(Ranked {
                        item,
                        matches,
                        tier,
                    });
                    None
                }
                None => Some(item),
            })
            .collect();
        accepted.sort_by(|a, b| {
            let fraction = |ranked: &Ranked<A>| {
                let matched: usize = ranked.matches.iter().map(|m| m.len()).sum();
                matched as f64 / key(&ranked.item).len().max(1) as f64
            };
            fraction(b).total_cmp(&fraction(a))
        });
        ranked.extend(accepted);
    }
    ranked
}

#[cfg(test)]
mod tests {
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
}
