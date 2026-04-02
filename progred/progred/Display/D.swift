import Foundation

typealias Commit = (Editor, Id?) -> Void

struct Descend {
    let inCycle: Bool
    let commit: Commit?
    let body: D
}

indirect enum D {
    // MARK: - Layout
    case block([D])
    case line([D])
    case indent(D)
    case bracketed(open: String, close: String, body: D)

    // MARK: - Content
    case text(String, TextStyle)
    case space
    case identicon(UUID)

    // MARK: - Structure
    case descend(Descend)
    case collapse(collapsed: Bool, header: D, body: () -> D)
    case list(separator: String, elements: [D])

    // MARK: - Interactive
    case placeholder
    case insertionPoint((Editor, Id) -> Void)
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
