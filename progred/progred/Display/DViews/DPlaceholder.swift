import AppKit

private class Pill: NSView, FocusTarget, StructuralNode {
    override var isFlipped: Bool { true }
    var onActivate: ((Bool) -> Void)?
    // Set in mouseDown, consumed in becomeFirstResponder. Relies on
    // makeFirstResponder synchronously calling becomeFirstResponder, so the
    // flag is read in the same call stack — don't add async between them.
    private var clickPending = false
    private var isHovered = false

    override var intrinsicContentSize: NSSize {
        let textHeight = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: ceil(textHeight), height: ceil(textHeight))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.separatorColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
        if isHovered {
            let inset = bounds.insetBy(dx: 0.75, dy: 0.75)
            NSColor.controlAccentColor.withAlphaComponent(0.5).setStroke()
            let stroke = NSBezierPath(roundedRect: inset, xRadius: 3, yRadius: 3)
            stroke.lineWidth = 1
            stroke.stroke()
        }
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach { removeTrackingArea($0) }
        addTrackingArea(NSTrackingArea(
            rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow],
            owner: self, userInfo: nil))
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        needsDisplay = true
    }

    override var acceptsFirstResponder: Bool { onActivate != nil }
    override var canBecomeKeyView: Bool { onActivate != nil && !isHiddenOrHasHiddenAncestor }

    var isTabTarget: Bool { onActivate != nil && !isHiddenOrHasHiddenAncestor }

    override func mouseDown(with event: NSEvent) {
        clickPending = true
        window?.makeFirstResponder(self)
    }

    override func becomeFirstResponder() -> Bool {
        let expanded = clickPending
        clickPending = false
        onActivate?(expanded)
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
    var advance: Advance?
    var focusBody: FocusBody?
    private let pill = Pill()
    private var searchBox: SearchBox?

    init(commit: ((Editor, Id) -> Void)?, expectedType: Id?, substitution: Substitution, editor: Editor, advance: Advance?, focusBody: FocusBody?) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        self.advance = advance
        self.focusBody = focusBody
        super.init(frame: .zero)
        pill.onActivate = commit != nil ? { [weak self] expanded in self?.activate(expanded: expanded) } : nil
        addSubview(pill)
        constrain(pill, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        searchBox?.intrinsicContentSize ?? pill.intrinsicContentSize
    }

    // Focus must move to the searchField before the pill is hidden. Hiding a
    // currently-first-responder view makes AppKit auto-advance to the next
    // valid key view, cascading through every activatable pill in the window.
    // If more AppKit weirdness shows up around this state toggle, consider
    // switching to remove/re-add (like InsertionPointView does with tabStop) —
    // removal nulls FR to the window rather than advancing.
    private func activate(expanded: Bool) {
        guard let commit else { return }
        assert(searchBox == nil, "activate called while a search box is already present")
        let box = SearchBox(
            commit: commit, expectedType: expectedType, substitution: substitution, editor: editor, advance: advance, focusBody: focusBody,
            initiallyExpanded: expanded,
            navAnchor: pill,
            onDismiss: { [weak self] in self?.dismissSearch() })
        self.searchBox = box
        addSubview(box)
        constrain(box, toFill: self)
        invalidateIntrinsicContentSize()
        box.focus()
        pill.isHidden = true
        // Explicit recalc — autorecalculatesKeyViewLoop doesn't reliably pick
        // up the isHidden change before the user's next Tab traversal.
        window?.recalculateKeyViewLoop()
        rescanInsertionZones()
    }

    private func dismissSearch() {
        guard let box = searchBox else { return }
        searchBox = nil
        box.removeFromSuperview()
        pill.isHidden = false
        invalidateIntrinsicContentSize()
        rescanInsertionZones()
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?, focusBody: FocusBody?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit.map { c in { editor, id in c(editor, id) } }
        self.expectedType = expectedType
        self.substitution = substitution
        self.advance = advance
        self.focusBody = focusBody
        pill.onActivate = self.commit != nil ? { [weak self] expanded in self?.activate(expanded: expanded) } : nil
        return true
    }
}
