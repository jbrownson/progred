import AppKit

final class Text: NSTextField {
    var style: TextStyle { didSet { textColor = style.nsColor } }
    var text: String {
        get { stringValue }
        set { stringValue = newValue }
    }

    init(_ text: String, _ style: TextStyle) {
        self.style = style
        super.init(frame: .zero)
        stringValue = text
        isEditable = false
        isSelectable = false
        isBordered = false
        isBezeled = false
        drawsBackground = false
        textColor = style.nsColor
        translatesAutoresizingMaskIntoConstraints = false
    }
    required init?(coder: NSCoder) { fatalError() }
}
