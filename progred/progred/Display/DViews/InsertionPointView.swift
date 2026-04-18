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
    private var searchBox: SearchBox?
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

    var isActive: Bool { searchBox != nil }

    override var intrinsicContentSize: NSSize {
        if let searchBox { return searchBox.intrinsicContentSize }
        return .zero
    }

    func update(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, advance: Advance?) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.advance = advance
    }

    func activate(expanded: Bool) {
        assert(searchBox == nil, "activate called while a search box is already present")
        let box = SearchBox(
            commit: commit, expectedType: expectedType, substitution: substitution, editor: editor, advance: advance,
            focusBody: { [weak self] in
                // After insertion, the new element is the next structural sibling of this IP.
                guard let self else { return }
                self.tabStop.nextFocusTarget(.down).flatMap { self.window?.makeFirstResponder($0) }
            },
            initiallyExpanded: expanded,
            navAnchor: tabStop,
            onDismiss: { [weak self] in self?.dismissSearch() })
        self.searchBox = box
        addSubview(box)
        constrain(box, toFill: self)
        invalidateIntrinsicContentSize()
        box.focus()
        onLayoutChange?()
        window?.recalculateKeyViewLoop()
        rescanInsertionZones()
    }

    private func dismissSearch() {
        searchBox?.removeFromSuperview()
        searchBox = nil
        isHovered = false
        invalidateIntrinsicContentSize()
        onLayoutChange?()
        rescanInsertionZones()
    }

}
