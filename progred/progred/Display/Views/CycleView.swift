import AppKit

/// Shown when a projection re-enters an entity that's already an ancestor —
/// avoids infinite recursion through self-referential graphs.
class CycleView: FlippedView, Projection {
    init(label: String) {
        super.init(frame: .zero)
        let stack = NSStackView(views: [
            styledLabel("↻", .keyword),
            styledLabel(label, .typeRef),
        ])
        stack.spacing = 2
        stack.orientation = .horizontal
        stack.alignment = .firstBaseline
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        constrain(stack, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {}
}
