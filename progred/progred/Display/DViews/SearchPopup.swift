import AppKit

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

class SearchPopup: FlippedView, NSTextFieldDelegate, NSTableViewDataSource, NSTableViewDelegate {
    let commit: (Editor, Id) -> Void
    let expectedType: Id?
    let editor: Editor
    let onDismiss: () -> Void
    private let searchField = SearchField()
    private let tableView: NSTableView
    private let scrollView: NSScrollView
    private let popupPanel: NSPanel
    private var filtered: [SearchResult] = []

    init(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, editor: Editor, onDismiss: @escaping () -> Void) {
        self.commit = commit
        self.expectedType = expectedType
        self.editor = editor
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
        window?.makeFirstResponder(searchField)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let window {
            repositionPanel()
            window.addChildWindow(popupPanel, ordered: .above)
        } else {
            popupPanel.parent?.removeChildWindow(popupPanel)
            popupPanel.orderOut(nil)
        }
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
        let entries = buildEntries(editor: editor, commit: commit, needle: needle, expectedType: expectedType)
        filtered = searchEntries(entries, needle: needle)

        tableView.reloadData()
        if !filtered.isEmpty {
            tableView.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        }
        if popupPanel.isVisible { repositionPanel() }
    }

    private func commitSelected() {
        guard filtered.indices.contains(tableView.selectedRow) else { return }
        filtered[tableView.selectedRow].entry.action(editor)
        onDismiss()
    }

    @objc private func tableViewClick() {
        commitSelected()
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
            commitSelected()
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            onDismiss()
            return true
        }
        if commandSelector == #selector(moveDown(_:)) {
            let next = min(tableView.selectedRow + 1, filtered.count - 1)
            tableView.selectRowIndexes(IndexSet(integer: next), byExtendingSelection: false)
            tableView.scrollRowToVisible(next)
            return true
        }
        if commandSelector == #selector(moveUp(_:)) {
            let prev = max(tableView.selectedRow - 1, 0)
            tableView.selectRowIndexes(IndexSet(integer: prev), byExtendingSelection: false)
            tableView.scrollRowToVisible(prev)
            return true
        }
        return false
    }

    func controlTextDidChange(_ obj: Notification) {
        rebuildEntries()
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        onDismiss()
    }
}
