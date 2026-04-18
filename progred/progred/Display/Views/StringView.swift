import AppKit

/// String literal display. Read-only for now; editing comes back when
/// commit plumbing is rebuilt.
class StringView: FlippedView, Projection {
    init(text: String) {
        super.init(frame: .zero)
        let open = styledLabel("\"", .literal)
        let label = styledLabel(text, .literal)
        let close = styledLabel("\"", .literal)
        let stack = NSStackView(views: [open, label, close])
        stack.spacing = 0
        stack.orientation = .horizontal
        stack.alignment = .firstBaseline
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        constrain(stack, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {}
}
