import AppKit

private class Pill: NSView {
    override var isFlipped: Bool { true }

    override var intrinsicContentSize: NSSize {
        let textHeight = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: ceil(textHeight), height: ceil(textHeight))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.separatorColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
    }
}

class DPlaceholder: FlippedView, Reconcilable {
    var commit: ((Editor, Id) -> Void)?
    let editor: Editor
    private let pill = Pill()
    private var searchPopup: SearchPopup?

    init(commit: ((Editor, Id) -> Void)?, editor: Editor) {
        self.commit = commit
        self.editor = editor
        super.init(frame: .zero)
        showPill()
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        searchPopup?.intrinsicContentSize ?? pill.intrinsicContentSize
    }

    override func mouseDown(with event: NSEvent) {
        guard commit != nil, searchPopup == nil else {
            nextResponder?.mouseDown(with: event)
            return
        }
        activate()
    }

    private func showPill() {
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(pill)
        constrain(pill, toFill: self)
        invalidateIntrinsicContentSize()
    }

    private func activate() {
        guard let commit else { return }
        let popup = SearchPopup(commit: commit, editor: editor) { [weak self] in
            self?.dismissSearch()
        }
        self.searchPopup = popup
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(popup)
        constrain(popup, toFill: self)
        invalidateIntrinsicContentSize()
        popup.focus()
    }

    private func dismissSearch() {
        searchPopup?.removeFromSuperview()
        searchPopup = nil
        showPill()
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .placeholder = d else { return false }
        self.commit = commit.map { c in { editor, id in c(editor, id) } }
        return true
    }
}
