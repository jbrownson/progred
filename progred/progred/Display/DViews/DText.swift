import AppKit

class DText: NSTextField, DView {
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

    func reconcile(_ d: D, editor: Editor) -> Bool {
        switch d {
        case .text(let s, let style):
            stringValue = s; textColor = style.nsColor
        case .placeholder:
            stringValue = "_"; textColor = NSColor.tertiaryLabelColor
        case .stringEditor(let s):
            stringValue = s; textColor = TextStyle.literal.nsColor
        case .numberEditor(let n):
            stringValue = String(n); textColor = TextStyle.literal.nsColor
        default:
            return false
        }
        return true
    }
}