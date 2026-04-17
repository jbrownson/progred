import AppKit

private class TabStop: NSView, FocusTarget, StructuralNode {
    override var isFlipped: Bool { true }
    var onActivate: (() -> Void)?
    var isInTabLoop: Bool = false

    override var intrinsicContentSize: NSSize { .zero }
    override var acceptsFirstResponder: Bool { onActivate != nil }
    override var canBecomeKeyView: Bool {
        onActivate != nil && !isHiddenOrHasHiddenAncestor && isInTabLoop
    }

    var isTabTarget: Bool {
        onActivate != nil && !isHiddenOrHasHiddenAncestor && isInTabLoop
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

class InsertionPointView: FlippedView {
    var commit: (Editor, Id) -> Void
    var expectedType: Id?
    var substitution: Substitution
    var advance: Advance?
    let editor: Editor
    var isHovered = false
    var vertical = false
    var onLayoutChange: (() -> Void)?
    var isTabReachable: Bool = false {
        didSet { tabStop.isInTabLoop = isTabReachable }
    }
    private var searchPopup: SearchPopup?
    private let tabStop = TabStop()

    init(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, editor: Editor, vertical: Bool, advance: Advance?) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        self.vertical = vertical
        self.advance = advance
        super.init(frame: .zero)
        tabStop.onActivate = { [weak self] in self?.activate(expanded: false) }
        addSubview(tabStop)
        constrain(tabStop, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    var isActive: Bool { searchPopup != nil }

    override var intrinsicContentSize: NSSize {
        if let searchPopup { return searchPopup.intrinsicContentSize }
        return .zero
    }

    func update(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, advance: Advance?) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.advance = advance
    }

    func activate(expanded: Bool) {
        assert(searchPopup == nil, "activate called while popup already present")
        let popup = SearchPopup(
            commit: commit, expectedType: expectedType, substitution: substitution, editor: editor, advance: advance,
            initiallyExpanded: expanded,
            navAnchor: tabStop,
            onDismiss: { [weak self] in self?.dismissSearch() })
        self.searchPopup = popup
        addSubview(popup)
        constrain(popup, toFill: self)
        invalidateIntrinsicContentSize()
        popup.focus()
        onLayoutChange?()
        window?.recalculateKeyViewLoop()
        rescanInsertionZones()
    }

    private func dismissSearch() {
        searchPopup?.removeFromSuperview()
        searchPopup = nil
        isHovered = false
        invalidateIntrinsicContentSize()
        onLayoutChange?()
        rescanInsertionZones()
    }

}
