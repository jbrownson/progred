import AppKit

class DCollapse: FlippedView, Reconcilable {
    enum BodyState {
        case pending(() -> D)
        case rendered(NSView)
    }

    let bodyContainer: FlippedView
    let collapseButton: CollapseButton
    let headerRow: NSStackView
    var header: NSView
    var body: BodyState
    var editor: Editor
    var inCycle: Bool
    var vertical: Bool?

    private var bodyConstraints: [NSLayoutConstraint] = []
    private var collapsedConstraint: NSLayoutConstraint!

    var advance: Advance?

    init(collapsed: Bool, header: D, body: @escaping () -> D, editor: Editor, inCycle: Bool, vertical: Bool?, advance: Advance?) {
        self.collapseButton = CollapseButton(collapsed: collapsed || inCycle)
        self.bodyContainer = FlippedView()
        self.body = .pending(body)
        self.editor = editor
        self.inCycle = inCycle
        self.vertical = vertical
        self.advance = advance
        let header = createView(header, editor: editor, inCycle: inCycle, vertical: vertical, advance: advance)
        self.header = header

        let headerRow = NSStackView(views: [header, collapseButton])
        headerRow.orientation = .horizontal
        headerRow.alignment = .top
        headerRow.spacing = 0
        headerRow.translatesAutoresizingMaskIntoConstraints = false
        self.headerRow = headerRow

        super.init(frame: .zero)

        addSubview(headerRow)
        addSubview(bodyContainer)
        bodyContainer.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            headerRow.topAnchor.constraint(equalTo: topAnchor),
            headerRow.leadingAnchor.constraint(equalTo: leadingAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: headerRow.trailingAnchor),
        ])

        bodyConstraints = [
            bodyContainer.topAnchor.constraint(equalTo: headerRow.bottomAnchor),
            bodyContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            bodyContainer.bottomAnchor.constraint(equalTo: bottomAnchor),
            trailingAnchor.constraint(greaterThanOrEqualTo: bodyContainer.trailingAnchor),
        ]
        collapsedConstraint = headerRow.bottomAnchor.constraint(equalTo: bottomAnchor)

        if collapseButton.isCollapsed {
            collapsedConstraint.isActive = true
        } else {
            NSLayoutConstraint.activate(bodyConstraints)
            renderBody()
        }

        collapseButton.onCollapsedChanged = { [weak self] collapsed in
            guard let self else { return }
            if collapsed {
                NSLayoutConstraint.deactivate(bodyConstraints)
                collapsedConstraint.isActive = true
                bodyContainer.isHidden = true
            } else {
                collapsedConstraint.isActive = false
                NSLayoutConstraint.activate(bodyConstraints)
                bodyContainer.isHidden = false
                renderBody()
            }
            rescanInsertionZones()
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    private func renderBody() {
        guard case .pending(let thunk) = body else { return }
        let bodyView = createView(thunk(), editor: editor, inCycle: inCycle, vertical: vertical, advance: advance)
        bodyContainer.addSubview(bodyView)
        constrain(bodyView, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        body = .rendered(bodyView)
    }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?) -> Bool {
        guard case .collapse(_, let header, let body) = d else { return false }
        self.editor = editor
        self.inCycle = inCycle
        self.vertical = vertical
        self.advance = advance

        let resolvedHeader = reconcileChild(self.header, header, editor: editor, inCycle: inCycle, vertical: vertical, advance: advance)
        if resolvedHeader !== self.header {
            let index = headerRow.arrangedSubviews.firstIndex(of: self.header) ?? 0
            headerRow.removeArrangedSubview(self.header)
            self.header.removeFromSuperview()
            headerRow.insertArrangedSubview(resolvedHeader, at: index)
            self.header = resolvedHeader
        }

        switch self.body {
        case .pending:
            self.body = .pending(body)
        case .rendered(let bodyView):
            let resolved = reconcileChild(bodyView, body(), editor: editor, inCycle: inCycle, vertical: vertical, advance: advance)
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
