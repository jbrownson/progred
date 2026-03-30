import Foundation
import HashTreeCollections

struct MutGid: Gid {
    private(set) var data: TreeDictionary<UUID, TreeDictionary<Id, Id>>

    init() {
        data = TreeDictionary()
    }

    var entities: some Sequence<UUID> {
        data.keys
    }

    func edges(entity: Id) -> Edges? {
        guard case .uuid(let uuid) = entity else { return nil }
        return data[uuid].map { Edges(data: $0, readOnly: false) }
    }

    mutating func set(entity: UUID, label: Id, value: Id) {
        var edges = data[entity] ?? TreeDictionary()
        edges[label] = value
        data[entity] = edges
    }

    mutating func commit(entity: UUID, label: Id, value: Id?) {
        if let value {
            set(entity: entity, label: label, value: value)
        } else {
            delete(entity: entity, label: label)
        }
    }

    mutating func delete(entity: UUID, label: Id) {
        guard var edges = data[entity] else { return }
        edges.removeValue(forKey: label)
        if edges.isEmpty {
            data.removeValue(forKey: entity)
        } else {
            data[entity] = edges
        }
    }

    mutating func merge(_ other: TreeDictionary<UUID, TreeDictionary<Id, Id>>) {
        for (entity, newEdges) in other {
            if var existing = data[entity] {
                existing.merge(newEdges, uniquingKeysWith: { _, new in new })
                data[entity] = existing
            } else {
                data[entity] = newEdges
            }
        }
    }

    func frozen() -> ImmGid {
        ImmGid(data: data)
    }

    mutating func retainEntities(_ keep: Set<Id>) {
        data = TreeDictionary(
            uniqueKeysWithValues: data.filter { key, _ in keep.contains(.uuid(key)) }
        )
    }

    mutating func purge(_ id: Id) {
        if case .uuid(let uuid) = id {
            data.removeValue(forKey: uuid)
        }
        data = TreeDictionary(
            uniqueKeysWithValues: data.compactMap { entity, edges -> (UUID, TreeDictionary<Id, Id>)? in
                let filtered = TreeDictionary(
                    uniqueKeysWithValues: edges.filter { _, v in v != id }
                )
                return filtered.isEmpty ? nil : (entity, filtered)
            }
        )
    }
}
