import AppKit

let indentWidth: CGFloat = 16
private let spacing: CGFloat = 4

class DRootView: FlippedView {
    let editor: Editor

    init(editor: Editor) {
        self.editor = editor
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError() }

    func rebuild(_ d: D) {
        let resolved = reconcileChild(subviews.first, d, editor: editor)
        if resolved !== subviews.first {
            subviews.forEach { $0.removeFromSuperview() }
            addSubview(resolved)
            resolved.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                resolved.topAnchor.constraint(equalTo: topAnchor, constant: 8),
                resolved.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            ])
        }
    }

    override func layout() {
        super.layout()
        guard let clipView = superview as? NSClipView else { return }
        let visible = clipView.bounds.size
        let needed = subviews.reduce(CGSize.zero) { size, sub in
            CGSize(width: max(size.width, sub.frame.maxX + 8),
                   height: max(size.height, sub.frame.maxY + 8))
        }
        frame.size = NSSize(
            width: max(visible.width, needed.width),
            height: max(visible.height, needed.height))
    }

    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { false }

    override func keyDown(with event: NSEvent) {
        interpretKeyEvents([event])
    }

    override func insertTab(_ sender: Any?) { window?.selectNextKeyView(self) }
    override func insertBacktab(_ sender: Any?) { window?.selectPreviousKeyView(self) }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }
}

func createView(_ d: D, editor: Editor, parentReadOnly: Bool = false, inCycle: Bool = false, commit: Commit? = nil) -> NSView {
    switch d {
    case .text(let text, let style): DText(text, style)
    case .space: DSpace(spacing)
    case .identicon(let uuid): DIdenticon(uuid: uuid)
    case .block(let children): DBlock(children: children, editor: editor, parentReadOnly: parentReadOnly)
    case .line(let children): DLine(children: children, editor: editor, parentReadOnly: parentReadOnly)
    case .list(_, let elements): DList(elements: elements, editor: editor, parentReadOnly: parentReadOnly)
    case .indent(let child): DIndent(child: child, editor: editor, parentReadOnly: parentReadOnly)
    case .descend(let descend):
        DDescend(descend, parentReadOnly: parentReadOnly, editor: editor)
    case .collapse(let collapsed, let header, let body):
        DCollapse(collapsed: collapsed, header: header, body: body, editor: editor, parentReadOnly: parentReadOnly, inCycle: inCycle)
    case .bracketed(let open, let close, let body):
        DBracketed(open: open, close: close, body: body, editor: editor, parentReadOnly: parentReadOnly)
    case .placeholder: DPlaceholder(commit: commit.map { c in { editor, id in c(editor, id) } }, editor: editor)
    case .insertionPoint(let commit): DInsertionPoint(commit: commit, editor: editor)
    case .stringEditor(let string): DStringEditor(string, editor: editor, readOnly: parentReadOnly, commit: commit)
    case .numberEditor(let number): DText(String(number), .literal)
    }
}
