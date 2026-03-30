import AppKit

class DPlaceholder: NSTextField, Reconcilable {
    var commit: Commit?
    weak var editor: Editor?

    init(commit: Commit?, editor: Editor) {
        self.commit = commit
        self.editor = editor
        super.init(frame: .zero)
        stringValue = "_"
        textColor = .tertiaryLabelColor
        isBezeled = false
        isEditable = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    override func mouseDown(with event: NSEvent) {
        guard let editor, let commit else {
            nextResponder?.mouseDown(with: event)
            return
        }
        commit(editor, .string("test"))
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit
        return true
    }
}
