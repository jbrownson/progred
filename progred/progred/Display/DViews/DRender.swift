import AppKit

let indentWidth: CGFloat = 16
private let spacing: CGFloat = 4

class DRootView: FlippedView {
    var editor: Editor
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
        insertionOverlay.rescan()
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

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
    }
}

func createView(_ d: D, editor: Editor, inCycle: Bool = false, commit: Commit? = nil, expectedType: Id? = nil, substitution: Substitution = .init(), vertical: Bool? = nil, advance: Advance? = nil, focusBody: FocusBody? = nil) -> NSView {
    switch d {
    case .text(let text, let style): DText(text, style)
    case .space: DSpace(spacing)
    case .identicon(let uuid): DIdenticon(uuid: uuid)
    case .block(let children): DBlock(children: children, editor: editor, advance: advance, focusBody: focusBody)
    case .line(let children): DLine(children: children, editor: editor, advance: advance, focusBody: focusBody)
    case .list(let list): DList(list, editor: editor, advance: advance, focusBody: focusBody)
    case .indent(let child): DIndent(child: child, editor: editor, vertical: vertical, advance: advance, focusBody: focusBody)
    case .selectable(let body): DSelectable(body, editor: editor, commit: commit, advance: advance, focusBody: focusBody)
    case .descend(let descend): DDescend(descend, editor: editor, vertical: vertical)
    case .collapse(let collapsed, let header, let body):
        DCollapse(collapsed: collapsed, header: header, body: body, editor: editor, inCycle: inCycle, vertical: vertical, advance: advance, focusBody: focusBody)
    case .placeholder:
        DPlaceholder(commit: commit.map { c in { editor, id in c(editor, id) } }, expectedType: expectedType, substitution: substitution, editor: editor, advance: advance, focusBody: focusBody)
    case .stringEditor(let string): DStringEditor(string, editor: editor, commit: commit, advance: advance)
    case .numberEditor(let number): DNumberEditor(number, editor: editor, commit: commit, advance: advance)
    }
}
