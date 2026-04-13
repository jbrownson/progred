import AppKit

private class Pill: NSView, FocusTarget, StructuralNode {
    override var isFlipped: Bool { true }
    var onActivate: (() -> Void)?

    override var intrinsicContentSize: NSSize {
        let textHeight = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: ceil(textHeight), height: ceil(textHeight))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.separatorColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
    }

    override var acceptsFirstResponder: Bool { onActivate != nil }
    override var canBecomeKeyView: Bool { onActivate != nil && !isHiddenOrHasHiddenAncestor }

    var isTabTarget: Bool { onActivate != nil && !isHiddenOrHasHiddenAncestor }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func becomeFirstResponder() -> Bool {
        onActivate?()
        return true
    }

    override func insertTab(_ sender: Any?) {
        nextFocusTarget(.tab).flatMap { window?.makeFirstResponder($0) }
    }

    override func insertBacktab(_ sender: Any?) {
        nextFocusTarget(.backtab).flatMap { window?.makeFirstResponder($0) }
    }
}

class DPlaceholder: FlippedView, Reconcilable {
    var commit: ((Editor, Id) -> Void)?
    var expectedType: Id?
    var substitution: Substitution
    var editor: Editor
    private let pill = Pill()
    private var searchPopup: SearchPopup?

    init(commit: ((Editor, Id) -> Void)?, expectedType: Id?, substitution: Substitution, editor: Editor) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        super.init(frame: .zero)
        pill.onActivate = commit != nil ? { [weak self] in self?.activate() } : nil
        addSubview(pill)
        constrain(pill, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        searchPopup?.intrinsicContentSize ?? pill.intrinsicContentSize
    }

    // Focus must move to the searchField before the pill is hidden. Hiding a
    // currently-first-responder view makes AppKit auto-advance to the next
    // valid key view, cascading through every activatable pill in the window.
    // If more AppKit weirdness shows up around this state toggle, consider
    // switching to remove/re-add (like InsertionPointView does with tabStop) —
    // removal nulls FR to the window rather than advancing.
    private func activate() {
        guard let commit else { return }
        assert(searchPopup == nil, "activate called while popup already present")
        let popup = SearchPopup(commit: commit, expectedType: expectedType, substitution: substitution, editor: editor) { [weak self] in
            self?.dismissSearch()
        }
        self.searchPopup = popup
        addSubview(popup)
        constrain(popup, toFill: self)
        invalidateIntrinsicContentSize()
        popup.focus()
        pill.isHidden = true
        // Explicit recalc — autorecalculatesKeyViewLoop doesn't reliably pick
        // up the isHidden change before the user's next Tab traversal.
        window?.recalculateKeyViewLoop()
        rescanInsertionZones()
    }

    private func dismissSearch() {
        guard let popup = searchPopup else { return }
        searchPopup = nil
        popup.removeFromSuperview()
        pill.isHidden = false
        invalidateIntrinsicContentSize()
        rescanInsertionZones()
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit.map { c in { editor, id in c(editor, id) } }
        self.expectedType = expectedType
        self.substitution = substitution
        pill.onActivate = self.commit != nil ? { [weak self] in self?.activate() } : nil
        return true
    }
}
