import AppKit

class InsertionPointView: FlippedView {
    var commit: (Editor, Id) -> Void
    var expectedType: Id?
    var substitution: Substitution
    let editor: Editor
    var isHovered = false
    var vertical = false
    var onLayoutChange: (() -> Void)?
    private var searchPopup: SearchPopup?

    init(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, editor: Editor, vertical: Bool) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        self.vertical = vertical
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError() }

    var isActive: Bool { searchPopup != nil }

    override var intrinsicContentSize: NSSize {
        if let searchPopup { return searchPopup.intrinsicContentSize }
        return .zero
    }

    func update(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
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
        onLayoutChange?()
        popup.focus()
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
