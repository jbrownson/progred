import AppKit

private class HeaderRow: NSStackView {}

class DCollapse: FlippedView, DView {
    let bodyContainer: FlippedView
    let collapseButton: CollapseButton
    var header: NSView

    init(defaultCollapsed: Bool, header: D, body: D, editor: Editor) {
        self.collapseButton = CollapseButton(collapsed: defaultCollapsed)
        self.bodyContainer = FlippedView()
        let header = createView(header, editor: editor)
        self.header = header
        super.init(frame: .zero)

        let headerRow = HeaderRow(views: [header, collapseButton])
        headerRow.orientation = .horizontal
        headerRow.alignment = .top
        headerRow.spacing = 0
        headerRow.translatesAutoresizingMaskIntoConstraints = false
        addSubview(headerRow)

        let bodyView = createView(body, editor: editor)
        bodyContainer.addSubview(bodyView)
        constrain(bodyView, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        bodyContainer.isHidden = defaultCollapsed
        addSubview(bodyContainer)
        bodyContainer.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            headerRow.topAnchor.constraint(equalTo: topAnchor),
            headerRow.leadingAnchor.constraint(equalTo: leadingAnchor),
            bodyContainer.topAnchor.constraint(equalTo: headerRow.bottomAnchor),
            bodyContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            bodyContainer.bottomAnchor.constraint(equalTo: bottomAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: headerRow.trailingAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: bodyContainer.trailingAnchor),
        ])

        let tight = widthAnchor.constraint(equalToConstant: 0)
        tight.priority = NSLayoutConstraint.Priority(1)
        tight.isActive = true

        collapseButton.onCollapsedChanged = { [weak self] collapsed in
            self?.bodyContainer.isHidden = collapsed
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor) -> Bool {
        guard case .collapse(_, let header, let body) = d else { return false }

        let resolvedHeader = reconcileChild(self.header, header, editor: editor)
        if resolvedHeader !== self.header {
            if let headerRow = self.header.superview as? HeaderRow,
               let index = headerRow.arrangedSubviews.firstIndex(of: self.header) {
                headerRow.removeArrangedSubview(self.header)
                self.header.removeFromSuperview()
                headerRow.insertArrangedSubview(resolvedHeader, at: index)
            }
            self.header = resolvedHeader
        }

        if let bodyView = bodyContainer.subviews.first {
            let resolvedBody = reconcileChild(bodyView, body, editor: editor)
            if resolvedBody !== bodyView {
                bodyView.removeFromSuperview()
                bodyContainer.addSubview(resolvedBody)
                constrain(resolvedBody, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
            }
        }
        return true
    }
}
