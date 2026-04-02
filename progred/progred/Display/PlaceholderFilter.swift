import Foundation

struct Match {
    let start: Int
    let length: Int
}

struct SearchResult {
    let entry: PlaceholderEntry
    let matches: [Match]
}

// MARK: - Filter pipeline

private typealias TextOf = (PlaceholderEntry) -> String
private typealias Filter = ([PlaceholderEntry], String) -> (accepted: [SearchResult], rejected: [PlaceholderEntry])

private func orFilter(_ a: @escaping Filter, _ b: @escaping Filter) -> Filter {
    { entries, needle in
        let first = a(entries, needle)
        let second = b(first.rejected, needle)
        return (first.accepted + second.accepted, second.rejected)
    }
}

private func orFilters(_ filters: [Filter]) -> Filter {
    filters.dropFirst().reduce(filters.first ?? { entries, _ in ([], entries) }) { orFilter($0, $1) }
}

private func predicateFilter(_ predicate: @escaping (String, String) -> [Match]?, textOf: @escaping TextOf = \.display) -> Filter {
    { entries, needle in
        entries.reduce(into: ([SearchResult](), [PlaceholderEntry]())) { result, entry in
            if let matches = predicate(needle, textOf(entry)) {
                result.0.append(SearchResult(entry: entry, matches: matches))
            } else {
                result.1.append(entry)
            }
        }
    }
}

private func sortedFilter(_ base: @escaping Filter) -> Filter {
    { entries, needle in
        var result = base(entries, needle)
        result.accepted.sort { percentMatched($0) > percentMatched($1) }
        return result
    }
}

private func percentMatched(_ result: SearchResult) -> Double {
    guard !result.entry.display.isEmpty else { return 0 }
    let matched = result.matches.reduce(0) { $0 + $1.length }
    return Double(matched) / Double(result.entry.display.count)
}

private func caseInsensitive(_ predicate: @escaping (String, String) -> [Match]?) -> Filter {
    sortedFilter(predicateFilter(
        { predicate($0.lowercased(), $1) },
        textOf: { $0.display.lowercased() }))
}

// MARK: - Matching predicates

private func prefixMatch(_ needle: String, _ haystack: String) -> [Match]? {
    haystack.hasPrefix(needle) ? [Match(start: 0, length: needle.count)] : nil
}

private func substringMatch(_ needle: String, _ haystack: String) -> [Match]? {
    guard let range = haystack.range(of: needle) else { return nil }
    return [Match(start: haystack.distance(from: haystack.startIndex, to: range.lowerBound), length: needle.count)]
}

private func fuzzyMatch(_ needle: String, _ haystack: String) -> [Match]? {
    var matches: [Match] = []
    var searchIndex = haystack.startIndex
    for char in needle {
        guard let found = haystack[searchIndex...].firstIndex(of: char) else { return nil }
        matches.append(Match(start: haystack.distance(from: haystack.startIndex, to: found), length: 1))
        searchIndex = haystack.index(after: found)
    }
    return matches
}

// MARK: - Default filter

func searchEntries(_ entries: [PlaceholderEntry], needle: String) -> [SearchResult] {
    let results: [SearchResult]
    if needle.isEmpty {
        results = entries.map { SearchResult(entry: $0, matches: []) }
    } else {
        let pipeline = orFilters([
            sortedFilter(predicateFilter(prefixMatch)),
            sortedFilter(predicateFilter(substringMatch)),
            caseInsensitive(prefixMatch),
            caseInsensitive(substringMatch),
            sortedFilter(predicateFilter(fuzzyMatch)),
            caseInsensitive(fuzzyMatch),
        ])
        results = pipeline(entries, needle).accepted
    }
    return results.sorted {
        ($0.entry.matching ? 1 : 0, $0.entry.magic ? 0 : 1) > ($1.entry.matching ? 1 : 0, $1.entry.magic ? 0 : 1)
    }
}
