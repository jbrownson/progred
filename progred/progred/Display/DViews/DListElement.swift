import AppKit

class DListElement: FlippedView, Reconcilable {
    var consPath: Path
    var readOnly: Bool
    var parentReadOnly: Bool
    weak var editor: Editor?

    init(consPath: Path, readOnly: Bool, parentReadOnly: Bool, editor: Editor, child: D) {
        self.consPath = consPath
        self.readOnly = readOnly
        self.parentReadOnly = parentReadOnly
        self.editor = editor
        super.init(frame: .zero)
        let childView = createView(child, editor: editor, parentReadOnly: readOnly)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .descendListElement(let consPath, let readOnly, let child) = d, let childView = subviews.first else { return false }
        self.consPath = consPath
        self.readOnly = readOnly
        self.parentReadOnly = parentReadOnly
        let resolved = reconcileChild(childView, child, editor: editor, parentReadOnly: readOnly)
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
