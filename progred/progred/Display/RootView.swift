import AppKit

class RootView: FlippedView {
    var editor: Editor {
        didSet { rebuild() }
    }
    private var content: NSView?

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
        content?.removeFromSuperview()
        let projection = projectValue(editor, [], editor.root)
        addSubview(projection)
        projection.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            projection.topAnchor.constraint(equalTo: topAnchor, constant: outerPadding),
            projection.leadingAnchor.constraint(equalTo: leadingAnchor, constant: outerPadding),
            bottomAnchor.constraint(greaterThanOrEqualTo: projection.bottomAnchor, constant: outerPadding),
            trailingAnchor.constraint(greaterThanOrEqualTo: projection.trailingAnchor, constant: outerPadding),
        ])
        content = projection
    }
}
