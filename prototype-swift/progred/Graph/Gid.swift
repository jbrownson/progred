import Foundation
import HashTreeCollections

struct Edges {
    let data: TreeDictionary<Id, Id>
    let readOnly: Bool

    subscript(_ label: Id) -> Id? { data[label] }
}

protocol Gid {
    func edges(entity: Id) -> Edges?
    func get(entity: Id, label: Id) -> Id?
}

extension Gid {
    func get(entity: Id, label: Id) -> Id? {
        edges(entity: entity)?[label]
    }
}

struct PrimitiveGid: Gid {
    let recordField: Id
    let stringRecord: Id
    let numberRecord: Id

    func edges(entity: Id) -> Edges? {
        switch entity {
        case .string: Edges(data: [recordField: stringRecord], readOnly: true)
        case .number: Edges(data: [recordField: numberRecord], readOnly: true)
        case .uuid: nil
        }
    }
}

struct StackedGid<Top: Gid, Bottom: Gid>: Gid {
    let top: Top
    let bottom: Bottom

    func edges(entity: Id) -> Edges? {
        top.edges(entity: entity) ?? bottom.edges(entity: entity)
    }
}
