import AppKit

class DDescend: FlippedView, Reconcilable {
    var descend: Descend
    var childView: NSView
    let editor: Editor

    init(_ descend: Descend, editor: Editor) {
        self.descend = descend
        self.childView = createView(descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit)
        self.editor = editor
        super.init(frame: .zero)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .descend(let descend) = d else { return false }
        self.descend = descend
        let resolved = reconcileChild(childView, descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self)
            childView = resolved
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
        guard isSelected else { return }
        let rect = bounds.insetBy(dx: -2, dy: -2)
        NSColor.selectedContentBackgroundColor.withAlphaComponent(0.3).setFill()
        NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3).fill()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        interpretKeyEvents([event])
    }

    @objc func delete(_ sender: Any?) {
        guard let commit = descend.commit else { return }
        window?.makeFirstResponder(nil)
        commit(editor, nil)
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(nil)
    }
}
