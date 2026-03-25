import AppKit

class FlippedView: NSView {
    override var isFlipped: Bool { true }
    override func mouseDown(with event: NSEvent) {
        nextResponder?.mouseDown(with: event)
    }
}

func constrain(_ child: NSView, toFill parent: NSView, insets: NSEdgeInsets = NSEdgeInsets()) {
    child.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
        child.topAnchor.constraint(equalTo: parent.topAnchor, constant: insets.top),
        child.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: insets.left),
        child.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -insets.right),
        child.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -insets.bottom),
    ])
}

func styledLabel(_ text: String, _ style: TextStyle) -> NSTextField {
    let field = NSTextField(labelWithString: text)
    field.textColor = style.nsColor
    field.font = .systemFont(ofSize: NSFont.systemFontSize)
    field.translatesAutoresizingMaskIntoConstraints = false
    return field
}

extension TextStyle {
    var nsColor: NSColor {
        switch self {
        case .keyword: .systemPurple
        case .typeRef: .systemCyan
        case .punctuation: .secondaryLabelColor
        case .label: .secondaryLabelColor
        case .literal: .labelColor
        }
    }
}
