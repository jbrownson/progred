import AppKit

class DStringEditor: NSTextField, Reconcilable, NSTextFieldDelegate {
    weak var editor: Editor?
    var path: Path
    var readOnly: Bool

    init(_ string: String, editor: Editor, path: Path, readOnly: Bool) {
        self.editor = editor
        self.path = path
        self.readOnly = readOnly
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
        guard let font else { return super.intrinsicContentSize }
        let width = ceil((stringValue as NSString).size(withAttributes: [.font: font]).width)
        return NSSize(width: width, height: super.intrinsicContentSize.height)
    }

    func controlTextDidChange(_ obj: Notification) {
        invalidateIntrinsicContentSize()
        guard !readOnly, let editor else { return }
        editor.handleSet(path: path, value: .string(stringValue))
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .stringEditor(let s) = d, let editPath else { return false }
        self.editor = editor
        self.path = editPath
        self.readOnly = parentReadOnly
        if currentEditor() == nil { stringValue = s }
        return true
    }
}
