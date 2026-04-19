import AppKit

final class Text: NSTextField {
    let focusable: Bool
    var style: TextStyle { didSet { textColor = style.nsColor } }
    var text: String {
        get { stringValue }
        set { stringValue = newValue }
    }

    init(_ text: String, _ style: TextStyle, focusable: Bool = true) {
        self.style = style
        self.focusable = focusable
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

    override var acceptsFirstResponder: Bool { focusable }

    override func mouseDown(with event: NSEvent) {
        if focusable {
            window?.makeFirstResponder(self)
        } else {
            nextResponder?.mouseDown(with: event)
        }
    }
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok { setFocusIndicator(true) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { setFocusIndicator(false) }
        return ok
    }
}
