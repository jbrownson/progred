import Foundation
import HashTreeCollections

struct Edges {
    let data: TreeDictionary<Id, Id>
    let readOnly: Bool

    subscript(_ label: Id) -> Id? { data[label] }
}

protocol Gid {
    func edges(entity: Id) -> Edges?
}

extension Gid {
    func get(entity: Id, label: Id) -> Id? {
        edges(entity: entity)?[label]
    }
}

struct StackedGid<Top: Gid, Bottom: Gid>: Gid {
    let top: Top
    let bottom: Bottom

    func edges(entity: Id) -> Edges? {
        top.edges(entity: entity) ?? bottom.edges(entity: entity)
    }
}
