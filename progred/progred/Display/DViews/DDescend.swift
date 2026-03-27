import AppKit

class DDescend: FlippedView, Reconcilable {
    var descend: Descend
    var parentReadOnly: Bool = false
    weak var editor: Editor?

    init(_ descend: Descend, parentReadOnly: Bool, editor: Editor) {
        self.descend = descend
        self.parentReadOnly = parentReadOnly
        self.editor = editor
        super.init(frame: .zero)
        let childView = createView(descend.body, editor: editor, parentReadOnly: descend.readOnly, editPath: descend.path)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .descend(let descend) = d, let childView = subviews.first else { return false }
        self.descend = descend
        self.parentReadOnly = parentReadOnly
        let resolved = reconcileChild(childView, descend.body, editor: editor, parentReadOnly: descend.readOnly, editPath: descend.path)
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
        } else if descend.readOnly, !parentReadOnly {
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
        guard let editor, let delete = descend.delete else { return }
        delete(editor)
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(superview)
    }
}
