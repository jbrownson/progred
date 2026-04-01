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

private func predicateFilter(_ predicate: @escaping (String, String) -> [Match]?) -> Filter {
    { entries, needle in
        var accepted: [SearchResult] = []
        var rejected: [PlaceholderEntry] = []
        for entry in entries {
            if let matches = predicate(needle, entry.display) {
                accepted.append(SearchResult(entry: entry, matches: matches))
            } else {
                rejected.append(entry)
            }
        }
        return (accepted, rejected)
    }
}

private func sortedFilter(_ base: @escaping Filter) -> Filter {
    { entries, needle in
        var result = base(entries, needle)
        result.accepted.sort { a, b in
            percentMatched(a) > percentMatched(b)
        }
        return result
    }
}

private func percentMatched(_ entry: SearchResult) -> Double {
    guard !entry.entry.display.isEmpty else { return 0 }
    let matched = entry.matches.reduce(0) { $0 + $1.length }
    return Double(matched) / Double(entry.entry.display.count)
}

private func caseInsensitive(_ base: @escaping Filter) -> Filter {
    { entries, needle in
        let lowerNeedle = needle.lowercased()
        // Create entries with lowercased display for matching, restore originals after
        let lowered = entries.map { ($0, $0.display.lowercased()) }
        var accepted: [SearchResult] = []
        var rejected: [PlaceholderEntry] = []
        for (entry, lowerDisplay) in lowered {
            let temp = PlaceholderEntry(display: lowerDisplay, disambiguation: entry.disambiguation, action: entry.action, matching: entry.matching, magic: entry.magic)
            let result = base([temp], lowerNeedle)
            if let filtered = result.accepted.first {
                accepted.append(SearchResult(entry: entry, matches: filtered.matches))
            } else {
                rejected.append(entry)
            }
        }
        return (accepted.sorted { percentMatched($0) > percentMatched($1) }, rejected)
    }
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
    if needle.isEmpty {
        return entries.map { SearchResult(entry: $0, matches: []) }
    }
    let pipeline = orFilters([
        sortedFilter(predicateFilter(prefixMatch)),
        sortedFilter(predicateFilter(substringMatch)),
        caseInsensitive(sortedFilter(predicateFilter(prefixMatch))),
        caseInsensitive(sortedFilter(predicateFilter(substringMatch))),
        sortedFilter(predicateFilter(fuzzyMatch)),
        caseInsensitive(sortedFilter(predicateFilter(fuzzyMatch))),
    ])
    return pipeline(entries, needle).accepted
}
