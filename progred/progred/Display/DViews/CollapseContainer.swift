import AppKit

class CollapseContainer: FlippedView {
    let bodyWrapper: NSView
    let toggle: TriangleButton

    init(defaultCollapsed: Bool, header: NSView, body: NSView) {
        self.toggle = TriangleButton(collapsed: defaultCollapsed)
        self.bodyWrapper = FlippedView()
        super.init(frame: .zero)

        let headerRow = hStack([header, toggle])

        bodyWrapper.addSubview(body)
        pin(body, to: bodyWrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        bodyWrapper.isHidden = defaultCollapsed

        let stack = vStack([headerRow, bodyWrapper])
        addSubview(stack)
        pin(stack, to: self)

        toggle.onToggle = { [weak self] collapsed in
            self?.bodyWrapper.isHidden = collapsed
        }
    }

    required init?(coder: NSCoder) { fatalError() }
}
