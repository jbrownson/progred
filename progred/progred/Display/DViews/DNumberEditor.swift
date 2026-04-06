import AppKit

class DNumberEditor: NSTextField, Reconcilable, NSTextFieldDelegate {
    let editor: Editor
    var commit: Commit?
    private var original: Double

    init(_ number: Double, editor: Editor, commit: Commit?) {
        self.editor = editor
        self.commit = commit
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

    override var intrinsicContentSize: NSSize {
        NSSize(width: textWidth(stringValue), height: super.intrinsicContentSize.height)
    }

    func controlTextDidChange(_ obj: Notification) {
        invalidateIntrinsicContentSize()
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(insertNewline(_:)) {
            if let value = Double(stringValue) {
                commit?(editor, .number(value))
                original = value
                window?.makeFirstResponder(nil)
            }
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            stringValue = String(original)
            invalidateIntrinsicContentSize()
            window?.makeFirstResponder(nil)
            return true
        }
        return false
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .numberEditor(let n) = d else { return false }
        self.commit = commit
        isEditable = commit != nil
        if currentEditor() == nil {
            original = n
            stringValue = String(n)
        }
        return true
    }
}
