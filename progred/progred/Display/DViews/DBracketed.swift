import AppKit

class DBracketed: FlippedView, Reconcilable {
    let bodyContainer: FlippedView
    let closeText: NSTextField
    let openText: NSTextField
    let collapseButton: CollapseButton
    var open: String
    var close: String

    init(open: String, close: String, body: D, editor: Editor, parentReadOnly: Bool) {
        self.open = open
        self.close = close
        self.collapseButton = CollapseButton(collapsed: false)
        self.openText = styledLabel(open, .punctuation)
        self.closeText = styledLabel(close, .punctuation)
        self.bodyContainer = FlippedView()
        super.init(frame: .zero)

        let bodyView = createView(body, editor: editor, parentReadOnly: parentReadOnly)
        bodyContainer.addSubview(bodyView)
        constrain(bodyView, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))

        let content: NSStackView = {
            let s = NSStackView(views: [openText, bodyContainer, closeText])
            s.orientation = .vertical; s.alignment = .leading; s.spacing = 0
            s.translatesAutoresizingMaskIntoConstraints = false
            return s
        }()
        let outer: NSStackView = {
            let s = NSStackView(views: [collapseButton, content])
            s.orientation = .horizontal; s.alignment = .top; s.spacing = 0
            s.translatesAutoresizingMaskIntoConstraints = false
            return s
        }()
        addSubview(outer)
        constrain(outer, toFill: self)

        collapseButton.onCollapsedChanged = { [weak self] collapsed in
            guard let self else { return }
            bodyContainer.isHidden = collapsed
            closeText.isHidden = collapsed
            openText.stringValue = collapsed ? "\(self.open)…\(self.close)" : self.open
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool) -> Bool {
        guard case .bracketed(let open, let close, let body) = d else { return false }
        self.open = open
        self.close = close
        openText.stringValue = collapseButton.isCollapsed ? "\(open)…\(close)" : open
        closeText.stringValue = close

        if let bodyView = bodyContainer.subviews.first {
            let resolved = reconcileChild(bodyView, body, editor: editor, parentReadOnly: parentReadOnly)
            if resolved !== bodyView {
                bodyView.removeFromSuperview()
                bodyContainer.addSubview(resolved)
                constrain(resolved, toFill: bodyContainer, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
            }
        }
        return true
    }
}
