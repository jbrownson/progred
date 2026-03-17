import SwiftUI

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
    case selectable(SelectionActions, child: D)
    case collapse(defaultCollapsed: Bool = false, header: D, body: D)
    case list(separator: String, elements: [D])

    // MARK: - Interactive
    case placeholder
    case stringEditor(String)
    case numberEditor(Double)
}

struct SelectionActions {
    var onDelete: (() -> Void)? = nil
}

enum TextStyle {
    case keyword
    case typeRef
    case punctuation
    case label
    case literal
}
