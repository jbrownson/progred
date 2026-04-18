import AppKit

class DList: FlippedView, Reconcilable {
    var list: List
    var editor: Editor
    var advance: Advance?
    var focusBody: FocusBody?
    private var elementViews: [NSView] = []
    private var insertionPoints: [InsertionPointView] = []

    // Vertical layout
    private var collapseButton: CollapseButton?
    private var openLabel: NSTextField?
    private var closeLabel: NSTextField?
    private var bodyStack: NSStackView?
    private var bodyContainer: FlippedView?

    // Inline layout
    private var lineStack: NSStackView?

    init(_ list: List, editor: Editor, advance: Advance?, focusBody: FocusBody?) {
        self.list = list
        self.editor = editor
        self.advance = advance
        self.focusBody = focusBody
        super.init(frame: .zero)

        elementViews = list.elements.map { createView($0, editor: editor, vertical: list.inline ? nil : true, advance: advance, focusBody: focusBody) }
        insertionPoints = makeInsertionPoints()

        if list.inline {
            setupInline()
        } else {
            setupVertical()
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    // MARK: - Insertion points

    private func makeInsertionPoints() -> [InsertionPointView] {
        guard let insertion = list.insertion else { return [] }
        let tabReachable = list.elements.isEmpty
        return (0...list.elements.count).map { position in
            let ip = InsertionPointView(
                commit: { editor, id in insertion.insert(editor, id, position) },
                expectedType: insertion.expectedType,
                substitution: insertion.substitution,
                editor: editor,
                vertical: !list.inline,
                advance: advance)
            ip.isTabReachable = tabReachable
            if list.inline { ip.onLayoutChange = { [weak self] in self?.populateInline() } }
            return ip
        }
    }

    private func updateInsertionPoints() {
        guard let insertion = list.insertion else {
            insertionPoints.forEach { $0.removeFromSuperview() }
            insertionPoints = []
            return
        }
        let needed = list.elements.count + 1
        let tabReachable = list.elements.isEmpty
        while insertionPoints.count > needed {
            let ip = insertionPoints.removeLast()
            ip.removeFromSuperview()
        }
        for (i, ip) in insertionPoints.enumerated() {
            let position = i
            ip.update(
                commit: { editor, id in insertion.insert(editor, id, position) },
                expectedType: insertion.expectedType,
                substitution: insertion.substitution,
                advance: advance)
            ip.vertical = !list.inline
            ip.isTabReachable = tabReachable
        }
        while insertionPoints.count < needed {
            let position = insertionPoints.count
            let ip = InsertionPointView(
                commit: { editor, id in insertion.insert(editor, id, position) },
                expectedType: insertion.expectedType,
                substitution: insertion.substitution,
                editor: editor,
                vertical: !list.inline,
                advance: advance)
            ip.isTabReachable = tabReachable
            if list.inline { ip.onLayoutChange = { [weak self] in self?.populateInline() } }
            insertionPoints.append(ip)
        }
    }

    // MARK: - Vertical layout

    private func setupVertical() {
        let collapse = CollapseButton(collapsed: false)
        self.collapseButton = collapse

        let open = styledLabel(list.open, .punctuation)
        self.openLabel = open

        let close = styledLabel(list.close, .punctuation)
        self.closeLabel = close

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.spacing = 0
        stack.alignment = .leading
        stack.translatesAutoresizingMaskIntoConstraints = false
        self.bodyStack = stack
        populateVerticalBody()

        let container = FlippedView()
        container.addSubview(stack)
        constrain(stack, toFill: container, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        self.bodyContainer = container

        let content = NSStackView(views: [open, container, close])
        content.orientation = .vertical
        content.alignment = .leading
        content.spacing = 0
        content.translatesAutoresizingMaskIntoConstraints = false

        let outer = NSStackView(views: [collapse, content])
        outer.orientation = .horizontal
        outer.alignment = .top
        outer.spacing = 0
        outer.translatesAutoresizingMaskIntoConstraints = false

        addSubview(outer)
        constrain(outer, toFill: self)

        collapse.onCollapsedChanged = { [weak self] collapsed in
            guard let self else { return }
            container.isHidden = collapsed
            close.isHidden = collapsed
            open.stringValue = collapsed ? "\(self.list.open)…\(self.list.close)" : self.list.open
            rescanInsertionZones()
        }
    }

    private func populateVerticalBody() {
        guard let stack = bodyStack else { return }
        stack.arrangedSubviews.forEach { stack.removeArrangedSubview($0); $0.removeFromSuperview() }
        for i in 0..<elementViews.count {
            if let ip = insertionPoints[safe: i] { stack.addArrangedSubview(ip) }
            stack.addArrangedSubview(elementViews[i])
        }
        if let trailing = insertionPoints[safe: elementViews.count] {
            stack.addArrangedSubview(trailing)
        }
    }

    // MARK: - Inline layout

    private func setupInline() {
        let stack = NSStackView()
        stack.orientation = .horizontal
        stack.spacing = 0
        stack.alignment = .top
        stack.translatesAutoresizingMaskIntoConstraints = false
        self.lineStack = stack

        addSubview(stack)
        constrain(stack, toFill: self)
        populateInline()
    }

    private func populateInline() {
        guard let stack = lineStack else { return }
        for view in stack.arrangedSubviews {
            stack.removeArrangedSubview(view)
            if !(view is InsertionPointView) && !elementViews.contains(where: { $0 === view }) {
                view.removeFromSuperview()
            }
        }
        stack.addArrangedSubview(styledLabel(list.open, .punctuation))
        var needsComma = false
        for i in 0..<elementViews.count {
            if let ip = insertionPoints[safe: i] {
                if ip.isActive && needsComma {
                    stack.addArrangedSubview(styledLabel(list.separator, .punctuation))
                    needsComma = false
                }
                stack.addArrangedSubview(ip)
                if ip.isActive { needsComma = true }
            }
            if needsComma {
                stack.addArrangedSubview(styledLabel(list.separator, .punctuation))
            }
            stack.addArrangedSubview(elementViews[i])
            needsComma = true
        }
        if let trailing = insertionPoints[safe: elementViews.count] {
            if trailing.isActive && needsComma {
                stack.addArrangedSubview(styledLabel(list.separator, .punctuation))
            }
            stack.addArrangedSubview(trailing)
        }
        stack.addArrangedSubview(styledLabel(list.close, .punctuation))
    }

    // MARK: - Reconcile

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?, focusBody: FocusBody?) -> Bool {
        guard case .list(let newList) = d, newList.inline == list.inline else { return false }
        self.editor = editor
        self.advance = advance
        self.focusBody = focusBody
        list = newList

        reconcileList(
            elementViews,
            with: newList.elements,
            reconcile: { existing, d in
                reconcileChild(existing, d, editor: editor, vertical: newList.inline ? nil : true, advance: advance, focusBody: focusBody)
            },
            replace: { i, _, new in self.elementViews[i] = new },
            append: { self.elementViews.append($0) },
            remove: { _ in self.elementViews.removeLast() })

        updateInsertionPoints()

        if list.inline {
            populateInline()
        } else {
            openLabel?.stringValue = collapseButton?.isCollapsed == true
                ? "\(list.open)…\(list.close)" : list.open
            closeLabel?.stringValue = list.close
            populateVerticalBody()
        }

        return true
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
