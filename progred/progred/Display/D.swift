import Foundation

indirect enum D: Equatable {
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
    case descend(Path, child: D)
    case descendListElement(consPath: Path, child: D)
    case collapse(defaultCollapsed: Bool = false, header: D, body: D)
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
