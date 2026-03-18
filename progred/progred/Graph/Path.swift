import Foundation

enum PathRoot: Hashable {
    case root
    case orphan(Id)
}

struct Path: Hashable {
    let root: PathRoot
    let edges: [Id]

    static func root() -> Path {
        Path(root: .root, edges: [])
    }

    static func orphan(_ id: Id) -> Path {
        Path(root: .orphan(id), edges: [])
    }

    func child(_ label: Id) -> Path {
        Path(root: root, edges: edges + [label])
    }

    func pop() -> (parent: Path, label: Id)? {
        guard let last = edges.last else { return nil }
        return (Path(root: root, edges: Array(edges.dropLast())), last)
    }

    func node(in gid: any Gid, root rootId: Id?) -> Id? {
        var current: Id
        switch root {
        case .root:
            guard let rootId else { return nil }
            current = rootId
        case .orphan(let id):
            current = id
        }
        for edge in edges {
            guard let next = gid.get(entity: current, label: edge) else { return nil }
            current = next
        }
        return current
    }
}
