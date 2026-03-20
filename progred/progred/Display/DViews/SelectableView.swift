import AppKit

class SelectableView: FlippedView {
    let path: Path
    weak var editor: Editor?

    init(path: Path, editor: Editor, child: NSView) {
        self.path = path
        self.editor = editor
        super.init(frame: .zero)
        addSubview(child)
        constrain(child, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    override func resignFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    private var isSelected: Bool {
        window?.firstResponder === self
    }

    override func draw(_ dirtyRect: NSRect) {
        guard isSelected else { return }
        NSColor.selectedContentBackgroundColor.withAlphaComponent(0.3).setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: -2, dy: -2), xRadius: 3, yRadius: 3).fill()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        switch Int(event.keyCode) {
        case 51: editor?.handleDelete(path: path) // Delete
        case 53: window?.makeFirstResponder(superview) // Escape
        default: super.keyDown(with: event)
        }
    }
}
