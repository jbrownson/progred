import AppKit

private class Caret: FlippedView {
    var onMouseDown: (() -> Void)?
    var isHovered = false

    override var intrinsicContentSize: NSSize {
        let textHeight = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: 8, height: ceil(textHeight))
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach { removeTrackingArea($0) }
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self))
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        guard isHovered else { return }
        let size: CGFloat = 6
        let path = NSBezierPath()
        let midX = bounds.midX
        let bottom = bounds.maxY
        path.move(to: NSPoint(x: midX - size / 2, y: bottom))
        path.line(to: NSPoint(x: midX, y: bottom - size))
        path.line(to: NSPoint(x: midX + size / 2, y: bottom))
        NSColor.secondaryLabelColor.setStroke()
        path.lineWidth = 1.5
        path.stroke()
    }

    override func mouseDown(with event: NSEvent) {
        onMouseDown?()
    }
}

class DInsertionPoint: FlippedView, Reconcilable {
    var commit: (Editor, Id) -> Void
    let editor: Editor
    private let caret = Caret()
    private var searchPopup: SearchPopup?

    init(commit: @escaping (Editor, Id) -> Void, editor: Editor) {
        self.commit = commit
        self.editor = editor
        super.init(frame: .zero)
        caret.onMouseDown = { [weak self] in self?.activate() }
        showCaret()
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        searchPopup?.intrinsicContentSize ?? caret.intrinsicContentSize
    }

    private func showCaret() {
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(caret)
        constrain(caret, toFill: self)
        invalidateIntrinsicContentSize()
    }

    private func activate() {
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
        showCaret()
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .insertionPoint(let commit) = d else { return false }
        self.commit = commit
        return true
    }
}
