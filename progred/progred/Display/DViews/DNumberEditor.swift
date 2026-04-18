import AppKit

class DNumberEditor: NSTextField, Reconcilable, NSTextFieldDelegate, StructuralNode {
    var editor: Editor
    var commit: Commit?
    var advance: Advance?
    private var original: Double

    init(_ number: Double, editor: Editor, commit: Commit?, advance: Advance?) {
        self.editor = editor
        self.commit = commit
        self.advance = advance
        self.original = number
        super.init(frame: .zero)
        stringValue = String(number)
        isBordered = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        textColor = TextStyle.literal.nsColor
        isEditable = commit != nil
        isSelectable = true
        delegate = self
        setContentHuggingPriority(.required, for: .horizontal)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    override var canBecomeKeyView: Bool { false }

    override var intrinsicContentSize: NSSize {
        NSSize(width: textWidth(stringValue), height: super.intrinsicContentSize.height)
    }

    func controlTextDidChange(_ obj: Notification) {
        invalidateIntrinsicContentSize()
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(deleteBackward(_:)) && textView.selectedRange().location == 0 && textView.selectedRange().length == 0 {
            commit?(editor, nil)
            return true
        }
        if commandSelector == #selector(deleteForward(_:)) && textView.selectedRange() == NSRange(location: stringValue.count, length: 0) {
            commit?(editor, nil)
            return true
        }
        if commandSelector == #selector(insertNewline(_:)) {
            if let value = Double(stringValue) {
                commit?(editor, .number(value))
                original = value
            }
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            stringValue = String(original)
            invalidateIntrinsicContentSize()
            window?.makeFirstResponder(nil)
            return true
        }
        if commandSelector == #selector(NSResponder.insertTab(_:)) {
            if let value = Double(stringValue) {
                commit?(editor, .number(value))
                original = value
            }
            advance?(.tab)
            return true
        }
        if commandSelector == #selector(NSResponder.insertBacktab(_:)) {
            if let value = Double(stringValue) {
                commit?(editor, .number(value))
                original = value
            }
            advance?(.backtab)
            return true
        }
        return false
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?, focusBody: FocusBody?) -> Bool {
        guard case .numberEditor(let n) = d else { return false }
        self.editor = editor
        self.commit = commit
        self.advance = advance
        isEditable = commit != nil
        if currentEditor() == nil {
            original = n
            stringValue = String(n)
        }
        return true
    }
}
