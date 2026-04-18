import AppKit

/// Number literal display. Read-only for now.
class NumberView: NSTextField, Projection {
    init(number: Double) {
        super.init(frame: .zero)
        stringValue = String(number)
        textColor = TextStyle.literal.nsColor
        isBezeled = false
        isEditable = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {}
}
