import Foundation
import HashTreeCollections

struct GraphDelta {
    let edges: TreeDictionary<UUID, TreeDictionary<Id, Id?>>

    static let empty = GraphDelta(edges: TreeDictionary())

    init(edges: TreeDictionary<UUID, TreeDictionary<Id, Id?>>) {
        self.edges = edges
    }

    static func setting(entity: UUID, label: Id, value: Id?) -> GraphDelta {
        GraphDelta(edges: TreeDictionary(uniqueKeysWithValues: [
            (entity, TreeDictionary(uniqueKeysWithValues: [(label, value)]))
        ]))
    }

    var isEmpty: Bool { edges.isEmpty }

    func affects(entity: UUID, label: Id) -> Bool {
        edges[entity]?[label] != nil
    }

    func affects(entity: UUID) -> Bool {
        edges[entity] != nil
    }

    func merging(_ other: GraphDelta) -> GraphDelta {
        var result = edges
        for (entity, entityEdges) in other.edges {
            result[entity] = result[entity].map { existing in
                var merged = existing
                merged.merge(entityEdges, uniquingKeysWith: { _, new in new })
                return merged
            } ?? entityEdges
        }
        return GraphDelta(edges: result)
    }
}
