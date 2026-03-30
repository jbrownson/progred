import AppKit

class DPlaceholder: NSTextField, Reconcilable, NSTextFieldDelegate {
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
        delegate = self
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        let text = isEditable ? (stringValue.isEmpty ? (placeholderString ?? "") : stringValue) : stringValue
        return NSSize(width: max(textWidth(text), 20), height: super.intrinsicContentSize.height)
    }

    override func textDidChange(_ notification: Notification) {
        super.textDidChange(notification)
        invalidateIntrinsicContentSize()
    }

    override func mouseDown(with event: NSEvent) {
        guard commit != nil else {
            nextResponder?.mouseDown(with: event)
            return
        }
        activate()
    }

    private func activate() {
        stringValue = ""
        textColor = .labelColor
        isEditable = true
        placeholderString = "search..."
        window?.makeFirstResponder(self)
    }

    private func deactivate() {
        stringValue = "_"
        textColor = .tertiaryLabelColor
        isEditable = false
        placeholderString = nil
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(insertNewline(_:)) {
            let text = stringValue
            if !text.isEmpty, let editor, let commit {
                commit(editor, .string(text))
            }
            deactivate()
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            deactivate()
            return true
        }
        return false
    }

    override func textDidEndEditing(_ notification: Notification) {
        super.textDidEndEditing(notification)
        if isEditable { deactivate() }
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit
        return true
    }
}
