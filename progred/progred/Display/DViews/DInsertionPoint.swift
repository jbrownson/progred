import AppKit

class DInsertionPoint: FlippedView, Reconcilable {
    var commit: (Editor, Id) -> Void
    var expectedType: Id?
    var substitution: Substitution
    let editor: Editor
    var isHovered = false
    var vertical = false
    private var searchPopup: SearchPopup?

    init(vertical: Bool?, commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, editor: Editor) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        self.vertical = vertical ?? false
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        if let searchPopup { return searchPopup.intrinsicContentSize }
        return .zero
    }

    func activate() {
        guard searchPopup == nil else { return }
        let popup = SearchPopup(commit: commit, expectedType: expectedType, substitution: substitution, editor: editor) { [weak self] in
            self?.dismissSearch()
        }
        self.searchPopup = popup
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(popup)
        constrain(popup, toFill: self)
        invalidateIntrinsicContentSize()
        popup.focus()
        rescanInsertionZones()
    }

    private func dismissSearch() {
        searchPopup?.removeFromSuperview()
        searchPopup = nil
        isHovered = false
        invalidateIntrinsicContentSize()
        rescanInsertionZones()
    }

    private func rescanInsertionZones() {
        var view: NSView? = self
        while let v = view {
            if let overlay = v as? InsertionOverlay { overlay.rescan(); return }
            view = v.superview
        }
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .insertionPoint(let commit, let expectedType, let substitution) = d else { return false }
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.vertical = vertical ?? false
        return true
    }
}
