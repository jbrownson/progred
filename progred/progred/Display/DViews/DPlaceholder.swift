import AppKit

private class Pill: NSView {
    static let pillHeight: CGFloat = 12
    override var isFlipped: Bool { true }

    override var intrinsicContentSize: NSSize {
        let textHeight = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: Self.pillHeight, height: ceil(textHeight))
    }

    override func draw(_ dirtyRect: NSRect) {
        let rect = NSRect(x: 0, y: (bounds.height - Self.pillHeight) / 2,
                          width: Self.pillHeight, height: Self.pillHeight)
        NSColor.separatorColor.setFill()
        NSBezierPath(roundedRect: rect, xRadius: 3, yRadius: 3).fill()
    }
}

private class SearchField: NSTextField {
    override var intrinsicContentSize: NSSize {
        let text = stringValue.isEmpty ? (placeholderString ?? "") : stringValue
        return NSSize(width: max(textWidth(text), 20), height: super.intrinsicContentSize.height)
    }
}

class DPlaceholder: FlippedView, Reconcilable, NSTextFieldDelegate, NSTableViewDataSource, NSTableViewDelegate {
    var commit: Commit?
    weak var editor: Editor?
    fileprivate let pill = Pill()
    fileprivate let searchField = SearchField()
    let tableView: NSTableView
    let scrollView: NSScrollView
    var popupPanel: NSPanel?
    var filtered: [SearchResult] = []

    init(commit: Commit?, editor: Editor) {
        self.commit = commit
        self.editor = editor

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

        super.init(frame: .zero)
        searchField.delegate = self
        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.doubleAction = #selector(tableViewDoubleClick)

        showPill()
    }

    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: NSSize {
        subviews.first?.intrinsicContentSize ?? pill.intrinsicContentSize
    }

    override func mouseDown(with event: NSEvent) {
        guard commit != nil else {
            nextResponder?.mouseDown(with: event)
            return
        }
        activate()
    }

    private func showPill() {
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(pill)
        constrain(pill, toFill: self)
        invalidateIntrinsicContentSize()
    }

    private func activate() {
        subviews.forEach { $0.removeFromSuperview() }
        searchField.stringValue = ""
        addSubview(searchField)
        constrain(searchField, toFill: self)
        invalidateIntrinsicContentSize()
        window?.makeFirstResponder(searchField)
        rebuildEntries()
        showPopup()
    }

    private func deactivate() {
        showPill()
        hidePopup()
    }

    private func showPopup() {
        guard let window else { return }
        let fieldRect = convert(bounds, to: nil)
        let screenRect = window.convertToScreen(fieldRect)
        let height = min(CGFloat(filtered.count) * tableView.rowHeight + 4, 300)
        let panelRect = NSRect(
            x: screenRect.minX,
            y: screenRect.minY - height - 2,
            width: max(screenRect.width, 200),
            height: height)

        if let panel = popupPanel {
            panel.setFrame(panelRect, display: true)
        } else {
            let panel = NSPanel(contentRect: panelRect, styleMask: [.borderless], backing: .buffered, defer: false)
            panel.isFloatingPanel = true
            panel.hasShadow = true
            panel.backgroundColor = .controlBackgroundColor
            panel.contentView = scrollView
            window.addChildWindow(panel, ordered: .above)
            self.popupPanel = panel
        }
    }

    private func hidePopup() {
        if let panel = popupPanel {
            panel.parent?.removeChildWindow(panel)
            panel.orderOut(nil)
            popupPanel = nil
        }
    }

    private func rebuildEntries() {
        guard let editor, let commit else { return }
        let needle = searchField.stringValue
        let entries = buildEntries(editor: editor, commit: commit, needle: needle)
        filtered = searchEntries(entries, needle: needle)

        filtered.sort {
            if $0.entry.matching != $1.entry.matching { return $0.entry.matching }
            if $0.entry.magic != $1.entry.magic { return !$0.entry.magic }
            return false
        }

        tableView.reloadData()
        if !filtered.isEmpty {
            tableView.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        }
        if popupPanel != nil { showPopup() }
    }

    private func commitSelected() {
        let row = tableView.selectedRow
        guard row >= 0, row < filtered.count, let editor else { return }
        filtered[row].entry.action(editor)
        deactivate()
    }

    @objc private func tableViewDoubleClick() {
        commitSelected()
    }

    override func removeFromSuperview() {
        hidePopup()
        super.removeFromSuperview()
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
            deactivate()
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
        searchField.invalidateIntrinsicContentSize()
        rebuildEntries()
    }

    func controlTextDidEndEditing(_ obj: Notification) {
        deactivate()
    }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .placeholder = d else { return false }
        self.editor = editor
        self.commit = commit
        return true
    }
}
