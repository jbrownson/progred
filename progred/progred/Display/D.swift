import Foundation

typealias Commit = (Editor, Id?) -> Void

struct Descend {
    let inCycle: Bool
    let commit: Commit?
    let expectedType: Id?
    let substitution: Substitution
    let body: D
}

struct ListInsert {
    let insert: (Editor, Id, Int) -> Void
    let expectedType: Id?
    let substitution: Substitution
}

struct List {
    let open: String
    let close: String
    let separator: String
    let inline: Bool
    let elements: [D]
    let insertion: ListInsert?
}

indirect enum D {
    // MARK: - Layout
    case block([D])
    case line([D])
    case indent(D)

    // MARK: - Content
    case text(String, TextStyle)
    case space
    case identicon(UUID)

    // MARK: - Structure
    case descend(Descend)
    case collapse(collapsed: Bool, header: D, body: () -> D)
    case list(List)

    // MARK: - Interactive
    case selectable(D)
    case placeholder
    case stringEditor(String)
    case numberEditor(Double)
}

enum TextStyle {
    case keyword
    case typeRef
    case punctuation
    case label
    case literal
}
