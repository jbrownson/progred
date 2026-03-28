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

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool) -> Bool {
        switch d {
        case .text(let s, let style):
            stringValue = s; textColor = style.nsColor
        case .placeholder:
            stringValue = "_"; textColor = NSColor.tertiaryLabelColor
        default:
            return false
        }
        return true
    }
}