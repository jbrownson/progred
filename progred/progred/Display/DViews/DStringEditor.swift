import AppKit

private class EditorField: NSTextField {
    override var canBecomeKeyView: Bool { false }

    override var intrinsicContentSize: NSSize {
        NSSize(width: max(textWidth(stringValue), 2), height: super.intrinsicContentSize.height)
    }

    override func textDidChange(_ notification: Notification) {
        super.textDidChange(notification)
        invalidateIntrinsicContentSize()
    }
}

class DStringEditor: FlippedView, Reconcilable, NSTextFieldDelegate {
    var editor: Editor
    var commit: Commit?
    private let field: NSTextField

    init(_ string: String, editor: Editor, commit: Commit?) {
        self.editor = editor
        self.commit = commit
        self.field = EditorField()
        super.init(frame: .zero)

        field.stringValue = string
        field.isBordered = false
        field.drawsBackground = false
        field.font = .systemFont(ofSize: NSFont.systemFontSize)
        field.textColor = TextStyle.literal.nsColor
        field.isEditable = commit != nil
        field.isSelectable = true
        field.delegate = self
        field.setContentHuggingPriority(.required, for: .horizontal)

        let open = styledLabel("\"", .literal)
        let close = styledLabel("\"", .literal)
        let stack = NSStackView(views: [open, field, close])
        stack.spacing = 0
        stack.orientation = .horizontal
        stack.alignment = .firstBaseline
        stack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(stack)
        constrain(stack, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(deleteBackward(_:)) && textView.selectedRange().location == 0 && textView.selectedRange().length == 0 {
            commit?(editor, nil)
            return true
        }
        if commandSelector == #selector(deleteForward(_:)) && textView.selectedRange() == NSRange(location: field.stringValue.count, length: 0) {
            commit?(editor, nil)
            return true
        }
        return false
    }

    func controlTextDidChange(_ obj: Notification) {
        field.invalidateIntrinsicContentSize()
        commit?(editor, .string(field.stringValue))
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .stringEditor(let s) = d else { return false }
        self.editor = editor
        self.commit = commit
        field.isEditable = commit != nil
        if field.currentEditor() == nil { field.stringValue = s }
        return true
    }
}
