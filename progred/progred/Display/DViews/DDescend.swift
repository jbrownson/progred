import AppKit

class DDescend: FlippedView, DView {
    var path: Path
    weak var editor: Editor?

    init(path: Path, editor: Editor, child: D) {
        self.path = path
        self.editor = editor
        super.init(frame: .zero)
        let childView = createView(child, editor: editor)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor) -> Bool {
        guard case .descend(let path, let child) = d, let childView = subviews.first else { return false }
        self.path = path
        let resolved = reconcileChild(childView, child, editor: editor)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self)
        }
        return true
    }

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
        if isSelected {
            NSColor.selectedContentBackgroundColor.withAlphaComponent(0.3).setFill()
            NSBezierPath(roundedRect: bounds.insetBy(dx: -2, dy: -2), xRadius: 3, yRadius: 3).fill()
        }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func deleteBackward(_ sender: Any?) {
        editor?.handleDelete(path: path)
    }

    override func deleteForward(_ sender: Any?) {
        editor?.handleDelete(path: path)
    }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(superview)
    }
}
