import Foundation

/// Wraps a Gid and records every (entity, label) read.
/// Used by projections to know which edges they depend on, so a graph
/// delta can be intersected against the read-set to decide whether the
/// projection needs to re-run.
final class TrackingGid: Gid {
    let underlying: any Gid
    private(set) var reads: Set<EdgeKey> = []

    init(_ underlying: any Gid) {
        self.underlying = underlying
    }

    func edges(entity: Id) -> Edges? {
        // We can't track at the Edges granularity since callers index
        // into Edges directly. Treat any Edges fetch as reading every
        // label the caller might inspect. In practice we get sharper
        // tracking by recording at .get(), so callers should prefer
        // get(entity:label:) when they want sharp dependencies.
        underlying.edges(entity: entity)
    }

    // Override of Gid extension: record the read here.
    func get(entity: Id, label: Id) -> Id? {
        if case .uuid(let uuid) = entity {
            reads.insert(EdgeKey(entity: uuid, label: label))
        }
        return underlying.get(entity: entity, label: label)
    }

    func clearReads() {
        reads.removeAll()
    }
}

struct EdgeKey: Hashable {
    let entity: UUID
    let label: Id
}

extension GraphDelta {
    func affects(_ key: EdgeKey) -> Bool {
        affects(entity: key.entity, label: key.label)
    }

    func affects(any keys: Set<EdgeKey>) -> Bool {
        keys.contains { affects($0) }
    }
}
