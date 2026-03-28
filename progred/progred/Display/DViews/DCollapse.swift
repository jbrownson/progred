import AppKit

private class HeaderRow: NSStackView {}

class DCollapse: FlippedView, Reconcilable {
    enum BodyState {
        case pending(() -> D)
        case rendered(NSView)
    }

    let bodyContainer: FlippedView
    let collapseButton: CollapseButton
    var header: NSView
    var body: BodyState
    var editor: Editor
    var parentReadOnly: Bool
    var inCycle: Bool

    init(collapsed: Bool, header: D, body: @escaping () -> D, editor: Editor, parentReadOnly: Bool, inCycle: Bool) {
        self.collapseButton = CollapseButton(collapsed: collapsed || inCycle)
        self.bodyContainer = FlippedView()
        self.body = .pending(body)
        self.editor = editor
        self.parentReadOnly = parentReadOnly
        self.inCycle = inCycle
        let header = createView(header, editor: editor, parentReadOnly: parentReadOnly, inCycle: inCycle)
        self.header = header
        super.init(frame: .zero)

        let headerRow = HeaderRow(views: [header, collapseButton])
        headerRow.orientation = .horizontal
        headerRow.alignment = .top
        headerRow.spacing = 0
        headerRow.translatesAutoresizingMaskIntoConstraints = false
        addSubview(headerRow)

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

        syncBody()

        collapseButton.onCollapsedChanged = { [weak self] _ in self?.syncBody() }
    }

    required init?(coder: NSCoder) { fatalError() }

    private func syncBody() {
        if collapseButton.isCollapsed {
            bodyContainer.isHidden = true
        } else {
            if case .pending(let thunk) = body {
                let bodyView = createView(thunk(), editor: editor, parentReadOnly: parentReadOnly, inCycle: inCycle)
                bodyContainer.addSubview(bodyView)
                constrain(bodyView, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
                body = .rendered(bodyView)
            }
            bodyContainer.isHidden = false
        }
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool) -> Bool {
        guard case .collapse(_, let header, let body) = d else { return false }
        self.editor = editor
        self.parentReadOnly = parentReadOnly
        self.inCycle = inCycle

        let resolvedHeader = reconcileChild(self.header, header, editor: editor, parentReadOnly: parentReadOnly, inCycle: inCycle)
        if resolvedHeader !== self.header {
            if let headerRow = self.header.superview as? HeaderRow,
               let index = headerRow.arrangedSubviews.firstIndex(of: self.header) {
                headerRow.removeArrangedSubview(self.header)
                self.header.removeFromSuperview()
                headerRow.insertArrangedSubview(resolvedHeader, at: index)
            }
            self.header = resolvedHeader
        }

        switch self.body {
        case .pending:
            self.body = .pending(body)
        case .rendered(let bodyView):
            let resolved = reconcileChild(bodyView, body(), editor: editor, parentReadOnly: parentReadOnly, inCycle: inCycle)
            if resolved !== bodyView {
                bodyView.removeFromSuperview()
                bodyContainer.addSubview(resolved)
                constrain(resolved, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
                self.body = .rendered(resolved)
            }
        }
        return true
    }
}
