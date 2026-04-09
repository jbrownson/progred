import Foundation

nonisolated enum Id: Hashable, Comparable, Sendable {
    case uuid(UUID)
    case string(String)
    case number(Double)

    static func newUUID() -> Id {
        .uuid(UUID())
    }

    var asUUID: UUID? {
        if case .uuid(let u) = self { u } else { nil }
    }

    var asString: String? {
        if case .string(let s) = self { s } else { nil }
    }

    static func < (lhs: Id, rhs: Id) -> Bool {
        switch (lhs, rhs) {
        case (.number(let a), .number(let b)): a < b
        case (.string(let a), .string(let b)): a < b
        case (.uuid(let a), .uuid(let b)): a.uuidString < b.uuidString
        case (.number, _): true
        case (.string, .uuid): true
        default: false
        }
    }
}
