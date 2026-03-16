import Foundation

indirect enum D {
    // Layout
    case block([D])
    case line([D])
    case indent(D)
    case bracketed(open: String, close: String, body: D)

    // Content
    case text(String, TextStyle)
    case identicon(UUID)

    // Structure
    case descend(label: Id, child: D)
    case collapse(collapsed: Bool, label: D, body: D)
    case list(separator: String, elements: [D])

    // Interactive (stubs)
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
