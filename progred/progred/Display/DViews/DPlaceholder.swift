import AppKit

class DPlaceholder: FlippedView, Reconcilable, NSTextFieldDelegate, NSTableViewDataSource, NSTableViewDelegate {
    var commit: Commit?
    weak var editor: Editor?
    let searchField: NSTextField
    let tableView: NSTableView
    let scrollView: NSScrollView
    var entries: [(display: String, action: () -> Void)] = []

    init(commit: Commit?, editor: Editor) {
        self.commit = commit
        self.editor = editor

        self.searchField = AutoSizingTextField()
        searchField.stringValue = "_"
        searchField.textColor = .tertiaryLabelColor
        searchField.isBezeled = false
        searchField.isEditable = false
        searchField.drawsBackground = false
        searchField.font = .systemFont(ofSize: NSFont.systemFontSize)
        searchField.translatesAutoresizingMaskIntoConstraints = false
        searchField.setContentHuggingPriority(.required, for: .horizontal)
        searchField.setContentCompressionResistancePriority(.required, for: .horizontal)

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("entry"))
        column.isEditable = false

        self.tableView = NSTableView()
        tableView.addTableColumn(column)
        tableView.headerView = nil
        tableView.style = .plain
        tableView.selectionHighlightStyle = .regular
        tableView.rowHeight = 20

        self.scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .lineBorder
        scrollView.isHidden = true
        scrollView.translatesAutoresizingMaskIntoConstraints = false

        super.init(frame: .zero)
        searchField.delegate = self
        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.doubleAction = #selector(tableViewDoubleClick)

        addSubview(searchField)
        addSubview(scrollView)

        NSLayoutConstraint.activate([
            searchField.topAnchor.constraint(equalTo: topAnchor),
            searchField.leadingAnchor.constraint(equalTo: leadingAnchor),
            searchField.trailingAnchor.constraint(equalTo: trailingAnchor),
            searchField.bottomAnchor.constraint(equalTo: bottomAnchor),
            scrollView.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: 2),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.widthAnchor.constraint(greaterThanOrEqualToConstant: 150),
            scrollView.heightAnchor.constraint(lessThanOrEqualToConstant: 200),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    override func mouseDown(with event: NSEvent) {
        guard commit != nil else {
            nextResponder?.mouseDown(with: event)
            return
        }
        activate()
    }

    private func activate() {
        searchField.stringValue = ""
        searchField.textColor = .labelColor
        searchField.isEditable = true
        searchField.placeholderString = "search..."
        searchField.invalidateIntrinsicContentSize()
        window?.makeFirstResponder(searchField)
        rebuildEntries()
        scrollView.isHidden = false
    }

    private func deactivate() {
        searchField.stringValue = "_"
        searchField.textColor = .tertiaryLabelColor
        searchField.isEditable = false
        searchField.placeholderString = nil
        searchField.invalidateIntrinsicContentSize()
        scrollView.isHidden = true
    }

    private func rebuildEntries() {
        entries = []
        let text = searchField.stringValue
        if !text.isEmpty {
            entries.append((display: "\"\(text)\"", action: { [weak self] in
                guard let self, let editor, let commit else { return }
                commit(editor, .string(text))
                deactivate()
            }))
        }
        tableView.reloadData()
        if !entries.isEmpty {
            tableView.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        }
        let height = min(CGFloat(max(entries.count, 1)) * tableView.rowHeight + 4, 200)
        scrollView.heightAnchor.constraint(equalToConstant: height).isActive = true
    }

    private func commitSelected() {
        let row = tableView.selectedRow
        guard row >= 0, row < entries.count else { return }
        entries[row].action()
    }

    @objc private func tableViewDoubleClick() {
        commitSelected()
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int { entries.count }

    // MARK: - NSTableViewDelegate

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = NSTextField(labelWithString: entries[row].display)
        cell.font = .systemFont(ofSize: NSFont.systemFontSize)
        return cell
    }

    // MARK: - NSTextFieldDelegate

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(insertNewline(_:)) {
            commitSelected()
            return true
        }
        if commandSelector == #selector(cancelOperation(_:)) {
            deactivate()
            return true
        }
        if commandSelector == #selector(moveDown(_:)) {
            let next = min(tableView.selectedRow + 1, entries.count - 1)
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
        searchField.invalidateIntrinsicContentSize()
        rebuildEntries()
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        if searchField.isEditable { deactivate() }
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit
        return true
    }
}

private class AutoSizingTextField: NSTextField {
    override var intrinsicContentSize: NSSize {
        let text = isEditable ? (stringValue.isEmpty ? (placeholderString ?? "") : stringValue) : stringValue
        return NSSize(width: max(textWidth(text), 20), height: super.intrinsicContentSize.height)
    }
}
