/// Entries sorted by key; a repeated key keeps its last value. Entries
/// already in strictly increasing key order stay as they are.
#[inline]
pub fn sorted_entries<K: Ord, V>(entries: impl IntoIterator<Item = (K, V)>) -> Vec<(K, V)> {
    let mut entries: Vec<_> = entries.into_iter().collect();
    if !entries.is_sorted_by(|(a, _), (b, _)| a < b) {
        entries.reverse();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        entries.dedup_by(|(a, _), (b, _)| a == b);
    }
    entries
}

#[cfg(test)]
#[path = "tests/entries.rs"]
mod tests;
