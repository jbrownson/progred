import AppKit

class RootView: FlippedView {
    let editor: Editor

    init(editor: Editor) {
        self.editor = editor
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        rebuild()
    }
    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {
        rebuild()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
    }

    private func rebuild() {
        subviews.first?.removeFromSuperview()
        assert(subviews.isEmpty)
        let editor = self.editor
        let rootCommit: Commit = { newValue in editor.setRoot(newValue) }
        let projection = Selectable(
            projectId(editor, [], editor.root, rootCommit),
            commit: rootCommit)
        addSubview(projection)
        projection.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            projection.topAnchor.constraint(equalTo: topAnchor, constant: outerPadding),
            projection.leadingAnchor.constraint(equalTo: leadingAnchor, constant: outerPadding),
            bottomAnchor.constraint(greaterThanOrEqualTo: projection.bottomAnchor, constant: outerPadding),
            trailingAnchor.constraint(greaterThanOrEqualTo: projection.trailingAnchor, constant: outerPadding),
        ])
    }
}
