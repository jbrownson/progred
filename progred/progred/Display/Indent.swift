import AppKit

final class Indent: FlippedView {
    init(_ child: NSView) {
        super.init(frame: .zero)
        addSubview(child)
        constrain(child, toFill: self, insets: NSEdgeInsets(
            top: 0, left: indentWidth, bottom: 0, right: 0))
    }
    required init?(coder: NSCoder) { fatalError() }
}
