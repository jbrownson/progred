import AppKit

class CollapseContainer: FlippedView {
    let bodyWrapper: FlippedView
    let toggle: TriangleButton

    init(defaultCollapsed: Bool, header: NSView, body: NSView) {
        self.toggle = TriangleButton(collapsed: defaultCollapsed)
        self.bodyWrapper = FlippedView()
        super.init(frame: .zero)

        let headerRow = hStack([header, toggle])
        addSubview(headerRow)
        headerRow.translatesAutoresizingMaskIntoConstraints = false

        bodyWrapper.addSubview(body)
        constrain(body, toFill: bodyWrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        bodyWrapper.isHidden = defaultCollapsed
        addSubview(bodyWrapper)
        bodyWrapper.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            headerRow.topAnchor.constraint(equalTo: topAnchor),
            headerRow.leadingAnchor.constraint(equalTo: leadingAnchor),
            bodyWrapper.topAnchor.constraint(equalTo: headerRow.bottomAnchor),
            bodyWrapper.leadingAnchor.constraint(equalTo: leadingAnchor),
            bodyWrapper.bottomAnchor.constraint(equalTo: bottomAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: headerRow.trailingAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: bodyWrapper.trailingAnchor),
        ])

        // Prefer tightest width that fits content
        let tight = widthAnchor.constraint(equalToConstant: 0)
        tight.priority = NSLayoutConstraint.Priority(1)
        tight.isActive = true

        toggle.onToggle = { [weak self] collapsed in
            self?.bodyWrapper.isHidden = collapsed
        }
    }

    required init?(coder: NSCoder) { fatalError() }
}
