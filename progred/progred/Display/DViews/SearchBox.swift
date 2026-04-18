import AppKit

// TODO: clicking the empty field in focused state should expand the popup.
// A mouseDown override here only fires for the first click that activates the
// field — once it's FR (e.g. via SearchBox.focus() on tab-in), clicks go to
// the field editor (an internal NSTextView), not here. Fix needs a custom
// field editor via NSWindowDelegate.windowWillReturnFieldEditor.
private class SearchField: NSTextField {
    override var intrinsicContentSize: NSSize {
        let text = stringValue.isEmpty ? (placeholderString ?? "") : stringValue
        return NSSize(width: max(textWidth(text), 20), height: super.intrinsicContentSize.height)
    }

    override func textDidChange(_ notification: Notification) {
        super.textDidChange(notification)
        invalidateIntrinsicContentSize()
    }
}

class SearchBox: FlippedView, NSTextFieldDelegate, NSTableViewDataSource, NSTableViewDelegate {
    let commit: (Editor, Id) -> Void
    let expectedType: Id?
    let substitution: Substitution
    let editor: Editor
    let advance: Advance?
    let onDismiss: () -> Void
    let navAnchor: NSView
    private let searchField = SearchField()
    private let tableView: NSTableView
    private let scrollView: NSScrollView
    private let popupPanel: NSPanel
    private var filtered: [SearchResult] = []
    private var isExpanded: Bool

    init(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, editor: Editor, advance: Advance?, initiallyExpanded: Bool, navAnchor: NSView, onDismiss: @escaping () -> Void) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
        self.editor = editor
        self.advance = advance
        self.isExpanded = initiallyExpanded
        self.navAnchor = navAnchor
        self.onDismiss = onDismiss

        searchField.isBezeled = false
        searchField.drawsBackground = false
        searchField.focusRingType = .none
        searchField.font = .systemFont(ofSize: NSFont.systemFontSize)
        searchField.placeholderString = "search..."
        searchField.setContentHuggingPriority(.required, for: .horizontal)

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("entry"))
        column.isEditable = false

        self.tableView = NSTableView()
        tableView.addTableColumn(column)
        tableView.headerView = nil
        tableView.style = .plain
        tableView.selectionHighlightStyle = .regular
        tableView.rowHeight = 20
        tableView.intercellSpacing = NSSize(width: 0, height: 0)

        self.scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .lineBorder

        self.popupPanel = NSPanel(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: true)
        popupPanel.isFloatingPanel = true
        popupPanel.hasShadow = true
        popupPanel.backgroundColor = .controlBackgroundColor
        popupPanel.contentView = scrollView

        super.init(frame: .zero)
        searchField.delegate = self
        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.action = #selector(tableViewClick)

        addSubview(searchField)
        constrain(searchField, toFill: self)
        rebuildEntries()
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize { searchField.intrinsicContentSize }

    override func layout() {
        super.layout()
        if popupPanel.isVisible { repositionPanel() }
    }

    func focus() {
        window!.makeFirstResponder(searchField)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            popupPanel.parent?.removeChildWindow(popupPanel)
            popupPanel.orderOut(nil)
        } else if isExpanded {
            showPanel()
        }
    }

    private func showPanel() {
        repositionPanel()
        window?.addChildWindow(popupPanel, ordered: .above)
    }

    private func expand() {
        guard !isExpanded else { return }
        isExpanded = true
        if window != nil { showPanel() }
    }

    private func collapse() {
        guard isExpanded else { return }
        isExpanded = false
        searchField.stringValue = ""
        searchField.invalidateIntrinsicContentSize()
        rebuildEntries()
        popupPanel.parent?.removeChildWindow(popupPanel)
        popupPanel.orderOut(nil)
    }

    private func navigateAway(_ direction: NavigationDirection) {
        let target = searchField.nextFocusTarget(direction)
        let win = window
        onDismiss()
        target.flatMap { win?.makeFirstResponder($0) }
    }

    private func navStructural(_ direction: NavigationDirection) {
        guard let target = navAnchor.nextFocusTarget(direction) else { return }
        let win = navAnchor.window
        onDismiss()
        win?.makeFirstResponder(target)
    }

    private let maxPanelHeight: CGFloat = 300
    private let minPanelWidth: CGFloat = 200
    private let panelGap: CGFloat = 2

    private func repositionPanel() {
        let screenRect = window!.convertToScreen(convert(bounds, to: nil))
        let contentHeight = CGFloat(filtered.count) * tableView.rowHeight
        let height = min(contentHeight + scrollView.contentInsets.top + scrollView.contentInsets.bottom, maxPanelHeight)
        popupPanel.setFrame(NSRect(
            x: screenRect.minX,
            y: screenRect.minY - height - panelGap,
            width: max(screenRect.width, minPanelWidth),
            height: height), display: true)
    }

    private func rebuildEntries() {
        let needle = searchField.stringValue
        let entries = buildEntries(editor: editor, commit: commit, needle: needle, expectedType: expectedType, substitution: substitution)
        filtered = searchEntries(entries, needle: needle)

        tableView.reloadData()
        if !filtered.isEmpty {
            tableView.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        }
        if popupPanel.isVisible { repositionPanel() }
    }

    private func commitSelected(advance direction: NavigationDirection) {
        if filtered.indices.contains(tableView.selectedRow) {
            filtered[tableView.selectedRow].entry.action(editor)
        }
        onDismiss()
        advance?(direction)
    }

    @objc private func tableViewClick() {
        commitSelected(advance: .tab)
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int { filtered.count }

    // MARK: - NSTableViewDelegate

    private let entryInset: CGFloat = 6

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let entry = filtered[row].entry
        let name = NSTextField(labelWithString: entry.display)
        name.font = .systemFont(ofSize: NSFont.systemFontSize)
        name.textColor = entry.matching ? .labelColor : .tertiaryLabelColor
        name.translatesAutoresizingMaskIntoConstraints = false

        let container = FlippedView()
        container.addSubview(name)
        NSLayoutConstraint.activate([
            name.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: entryInset),
            name.centerYAnchor.constraint(equalTo: container.centerYAnchor),
        ])

        if let dis = entry.disambiguation {
            let disLabel = NSTextField(labelWithString: dis)
            disLabel.font = .systemFont(ofSize: NSFont.smallSystemFontSize - 1)
            disLabel.textColor = .tertiaryLabelColor
            disLabel.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(disLabel)
            NSLayoutConstraint.activate([
                disLabel.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -entryInset),
                disLabel.centerYAnchor.constraint(equalTo: container.centerYAnchor),
                disLabel.leadingAnchor.constraint(greaterThanOrEqualTo: name.trailingAnchor, constant: 8),
            ])
        }

        return container
    }

    // MARK: - NSTextFieldDelegate

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(insertNewline(_:)) {
            if isExpanded { commitSelected(advance: .right) } else { expand() }
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            if isExpanded { collapse() } else { onDismiss() }
            return true
        }
        if commandSelector == #selector(moveDown(_:)) {
            if isExpanded {
                let next = min(tableView.selectedRow + 1, filtered.count - 1)
                tableView.selectRowIndexes(IndexSet(integer: next), byExtendingSelection: false)
                tableView.scrollRowToVisible(next)
            } else {
                navStructural(.down)
            }
            return true
        }
        if commandSelector == #selector(moveUp(_:)) {
            if isExpanded {
                let prev = max(tableView.selectedRow - 1, 0)
                tableView.selectRowIndexes(IndexSet(integer: prev), byExtendingSelection: false)
                tableView.scrollRowToVisible(prev)
            } else {
                navStructural(.up)
            }
            return true
        }
        if commandSelector == #selector(moveLeft(_:)) {
            if !isExpanded {
                navStructural(.left)
                return true
            }
            return false
        }
        if commandSelector == #selector(moveRight(_:)) {
            if !isExpanded {
                navStructural(.right)
                return true
            }
            return false
        }
        if commandSelector == #selector(NSResponder.insertTab(_:)) {
            if isExpanded { commitSelected(advance: .tab) } else { navigateAway(.tab) }
            return true
        }
        if commandSelector == #selector(NSResponder.insertBacktab(_:)) {
            if isExpanded { commitSelected(advance: .backtab) } else { navigateAway(.backtab) }
            return true
        }
        return false
    }

    func controlTextDidChange(_ obj: Notification) {
        expand()
        rebuildEntries()
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        onDismiss()
    }
}
