import Foundation
import HashTreeCollections

struct ImmGid: Gid {
    let data: TreeDictionary<UUID, TreeDictionary<Id, Id>>

    func edges(entity: Id) -> Edges? {
        guard case .uuid(let uuid) = entity else { return nil }
        return data[uuid].map { Edges(data: $0, readOnly: true) }
    }
}
