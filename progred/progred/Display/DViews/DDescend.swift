import AppKit

class DDescend: FlippedView, Reconcilable {
    var path: Path
    var readOnly: Bool
    var parentReadOnly: Bool = false
    weak var editor: Editor?

    init(path: Path, readOnly: Bool, parentReadOnly: Bool, editor: Editor, child: D) {
        self.path = path
        self.readOnly = readOnly
        self.parentReadOnly = parentReadOnly
        self.editor = editor
        super.init(frame: .zero)
        let childView = createView(child, editor: editor, parentReadOnly: readOnly, editPath: path)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .descend(let path, let readOnly, let child) = d, let childView = subviews.first else { return false }
        self.path = path
        self.readOnly = readOnly
        self.parentReadOnly = parentReadOnly
        let resolved = reconcileChild(childView, child, editor: editor, parentReadOnly: readOnly, editPath: path)
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
        let rect = bounds.insetBy(dx: -2, dy: -2)
        if isSelected {
            NSColor.selectedContentBackgroundColor.withAlphaComponent(0.3).setFill()
            NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3).fill()
        } else if readOnly, !parentReadOnly {
            NSColor.windowBackgroundColor.shadow(withLevel: 0.04)!.setFill()
            NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3).fill()
        }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        interpretKeyEvents([event])
    }

    @objc func delete(_ sender: Any?) {
        editor?.handleDelete(path: path)
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(superview)
    }
}
