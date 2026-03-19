import AppKit

class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

func label(_ text: String, color: NSColor) -> NSTextField {
    let field = NSTextField(labelWithString: text)
    field.textColor = color
    field.font = .systemFont(ofSize: NSFont.systemFontSize)
    field.translatesAutoresizingMaskIntoConstraints = false
    return field
}

func punctuationLabel(_ text: String) -> NSTextField {
    label(text, color: TextStyle.punctuation.nsColor)
}

func spacer(_ size: CGFloat) -> NSView {
    let view = NSView()
    view.translatesAutoresizingMaskIntoConstraints = false
    view.widthAnchor.constraint(equalToConstant: size).isActive = true
    view.heightAnchor.constraint(equalToConstant: size).isActive = true
    return view
}

func vStack(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 0
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

func hStack(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.alignment = .top
    stack.spacing = 0
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

func pin(_ child: NSView, to parent: NSView, insets: NSEdgeInsets = NSEdgeInsets()) {
    child.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
        child.topAnchor.constraint(equalTo: parent.topAnchor, constant: insets.top),
        child.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: insets.left),
        child.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -insets.right),
        child.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -insets.bottom),
    ])
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
