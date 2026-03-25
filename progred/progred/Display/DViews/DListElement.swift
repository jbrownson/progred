import AppKit

class DListElement: FlippedView, DView {
    var consPath: Path
    weak var editor: Editor?

    init(consPath: Path, editor: Editor, child: D) {
        self.consPath = consPath
        self.editor = editor
        super.init(frame: .zero)
        let childView = createView(child, editor: editor)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor) -> Bool {
        guard case .descendListElement(let consPath, let child) = d, let childView = subviews.first else { return false }
        self.consPath = consPath
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

    override func keyDown(with event: NSEvent) {
        interpretKeyEvents([event])
    }

    @objc func delete(_ sender: Any?) {
        spliceOut()
    }

    override func deleteBackward(_ sender: Any?) { delete(sender) }
    override func deleteForward(_ sender: Any?) { delete(sender) }

    override func cancelOperation(_ sender: Any?) {
        window?.makeFirstResponder(superview)
    }

    private func spliceOut() {
        guard let editor,
              case .uuid(let consUuid) = consPath.node(in: editor.gid, root: editor.root),
              let tail = editor.gid.get(entity: .uuid(consUuid), label: editor.schema.tailField),
              let (parentPath, edgeLabel) = consPath.pop(),
              case .uuid(let parentUuid) = parentPath.node(in: editor.gid, root: editor.root)
        else { return }
        editor.set(entity: parentUuid, label: edgeLabel, value: tail)
    }
}
