import AppKit

/// Static text view for keywords, labels, punctuation, etc. Not a projection
/// itself — wrapped by other projections that own the structural meaning.
class TextView: NSTextField, Projection {
    init(text: String, style: TextStyle) {
        super.init(frame: .zero)
        stringValue = text
        textColor = style.nsColor
        isBezeled = false
        isEditable = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {}
}
