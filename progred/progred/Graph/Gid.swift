import Foundation
import HashTreeCollections

protocol Gid {
    func edges(entity: Id) -> TreeDictionary<Id, Id>?

    func get(entity: Id, label: Id) -> Id?
}

extension Gid {
    func get(entity: Id, label: Id) -> Id? {
        edges(entity: entity)?[label]
    }
}

struct StackedGid<Top: Gid, Bottom: Gid>: Gid {
    let top: Top
    let bottom: Bottom

    func edges(entity: Id) -> TreeDictionary<Id, Id>? {
        top.edges(entity: entity) ?? bottom.edges(entity: entity)
    }
}
