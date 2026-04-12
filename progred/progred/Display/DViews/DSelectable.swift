import AppKit

class DSelectable: FlippedView, Reconcilable {
    var commit: Commit?
    var childView: NSView
    var editor: Editor

    init(_ body: D, editor: Editor, commit: Commit?) {
        self.commit = commit
        self.editor = editor
        self.childView = createView(body, editor: editor)
        super.init(frame: .zero)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .selectable(let body) = d else { return false }
        self.editor = editor
        self.commit = commit
        let resolved = reconcileChild(childView, body, editor: editor)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self)
            childView = resolved
        }
        return true
    }

    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { false }

    override func becomeFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    override func resignFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    override func draw(_ dirtyRect: NSRect) {
        guard window?.firstResponder === self else { return }
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
        guard let commit else { return }
        window?.makeFirstResponder(nil)
        commit(editor, nil)
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(nil)
    }
}
