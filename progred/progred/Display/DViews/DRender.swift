import AppKit

let indentWidth: CGFloat = 16
private let spacing: CGFloat = 4

class DRootView: FlippedView {
    let editor: Editor
    private let insertionOverlay = InsertionOverlay()

    init(editor: Editor) {
        self.editor = editor
        super.init(frame: .zero)
        addSubview(insertionOverlay)
        insertionOverlay.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            insertionOverlay.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            insertionOverlay.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        guard let clipView = superview as? NSClipView else { return }
        clipView.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(
            self, selector: #selector(clipViewFrameChanged),
            name: NSView.frameDidChangeNotification, object: clipView)
    }

    @objc private func clipViewFrameChanged(_ notification: Notification) {
        needsLayout = true
    }

    func rebuild(_ d: D) {
        let resolved = reconcileChild(insertionOverlay.subviews.first, d, editor: editor)
        if resolved !== insertionOverlay.subviews.first {
            insertionOverlay.subviews.forEach { $0.removeFromSuperview() }
            insertionOverlay.addSubview(resolved)
            constrain(resolved, toFill: insertionOverlay)
        }
    }

    override func layout() {
        super.layout()
        guard let clipView = superview as? NSClipView else { return }
        let visible = clipView.bounds.size
        let needed = CGSize(
            width: insertionOverlay.frame.maxX + 8,
            height: insertionOverlay.frame.maxY + 8)
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

func createView(_ d: D, editor: Editor, inCycle: Bool = false, commit: Commit? = nil, vertical: Bool? = nil) -> NSView {
    switch d {
    case .text(let text, let style): DText(text, style)
    case .space: DSpace(spacing)
    case .identicon(let uuid): DIdenticon(uuid: uuid)
    case .block(let children): DBlock(children: children, editor: editor)
    case .line(let children): DLine(children: children, editor: editor)
    case .list(_, let elements): DList(elements: elements, editor: editor)
    case .indent(let child): DIndent(child: child, editor: editor, vertical: vertical)
    case .descend(let descend): DDescend(descend, editor: editor, vertical: vertical)
    case .collapse(let collapsed, let header, let body):
        DCollapse(collapsed: collapsed, header: header, body: body, editor: editor, inCycle: inCycle, vertical: vertical)
    case .bracketed(let open, let close, let body):
        DBracketed(open: open, close: close, body: body, editor: editor, vertical: vertical)
    case .placeholder: DPlaceholder(commit: commit.map { c in { editor, id in c(editor, id) } }, editor: editor)
    case .insertionPoint(let commit): DInsertionPoint(vertical: vertical, commit: commit, editor: editor)
    case .stringEditor(let string): DStringEditor(string, editor: editor, commit: commit)
    case .numberEditor(let number): DNumberEditor(number, editor: editor, commit: commit)
    }
}
