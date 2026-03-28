import Foundation

struct Descend {
    let path: Path
    let readOnly: Bool
    let inCycle: Bool
    let delete: ((Editor) -> Void)?
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
