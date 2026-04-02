import AppKit

class DStringEditor: NSTextField, Reconcilable, NSTextFieldDelegate {
    let editor: Editor
    var commit: Commit?

    init(_ string: String, editor: Editor, readOnly: Bool, commit: Commit?) {
        self.editor = editor
        self.commit = commit
        super.init(frame: .zero)
        stringValue = string
        isBordered = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        textColor = TextStyle.literal.nsColor
        isEditable = !readOnly
        isSelectable = true
        delegate = self
        setContentHuggingPriority(.required, for: .horizontal)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        NSSize(width: textWidth(stringValue), height: super.intrinsicContentSize.height)
    }

    func controlTextDidChange(_ obj: Notification) {
        invalidateIntrinsicContentSize()
        commit?(editor, .string(stringValue))
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .stringEditor(let s) = d else { return false }
        self.commit = commit
        isEditable = !parentReadOnly
        if currentEditor() == nil { stringValue = s }
        return true
    }
}
