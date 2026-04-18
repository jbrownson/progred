import AppKit

class DSelectable: FlippedView, Reconcilable, FocusTarget, StructuralNode {
    var commit: Commit?
    var advance: Advance?
    var childView: NSView
    var editor: Editor

    var isTabTarget: Bool { false }

    var focusBody: FocusBody?

    init(_ body: D, editor: Editor, commit: Commit?, advance: Advance?, focusBody: FocusBody?) {
        self.commit = commit
        self.advance = advance
        self.focusBody = focusBody
        self.editor = editor
        self.childView = createView(body, editor: editor, advance: advance)
        super.init(frame: .zero)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?, focusBody: FocusBody?) -> Bool {
        guard case .selectable(let body) = d else { return false }
        self.editor = editor
        self.commit = commit
        self.advance = advance
        self.focusBody = focusBody
        let resolved = reconcileChild(childView, body, editor: editor, advance: advance)
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

    override func insertTab(_ sender: Any?) {
        nextFocusTarget(.tab).flatMap { window?.makeFirstResponder($0) }
    }

    override func insertBacktab(_ sender: Any?) {
        nextFocusTarget(.backtab).flatMap { window?.makeFirstResponder($0) }
    }

    override func moveUp(_ sender: Any?) {
        nextFocusTarget(.up).flatMap { window?.makeFirstResponder($0) }
    }

    override func moveDown(_ sender: Any?) {
        nextFocusTarget(.down).flatMap { window?.makeFirstResponder($0) }
    }

    override func moveLeft(_ sender: Any?) {
        nextFocusTarget(.left).flatMap { window?.makeFirstResponder($0) }
    }

    override func moveRight(_ sender: Any?) {
        nextFocusTarget(.right).flatMap { window?.makeFirstResponder($0) }
    }

    @objc func delete(_ sender: Any?) {
        guard let commit else { return }
        let focusBody = self.focusBody
        let prevSibling = nextFocusTarget(.up)
        commit(editor, nil)
        if let window {
            // self was kept (e.g. list element reused for the next element);
            // focus the previous structural — the insertion point at this slot.
            window.makeFirstResponder(prevSibling!)
        } else {
            // self was replaced (non-list: slot became a placeholder);
            // focusBody walks into the new body and lands on the new placeholder.
            focusBody?()
        }
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(nil)
    }
}
