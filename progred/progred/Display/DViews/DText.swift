import AppKit

class DText: NSTextField, Reconcilable {
    init(_ text: String, _ style: TextStyle) {
        super.init(frame: .zero)
        stringValue = text
        textColor = style.nsColor
        isBezeled = false
        isEditable = false
        drawsBackground = false
        font = .systemFont(ofSize: NSFont.systemFontSize)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .text(let s, let style) = d else { return false }
        stringValue = s; textColor = style.nsColor
        return true
    }
}