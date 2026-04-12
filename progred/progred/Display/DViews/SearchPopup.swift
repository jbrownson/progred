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
    let substitution: Substitution
    let editor: Editor
    let onDismiss: () -> Void
    private let searchField = SearchField()
    private let tableView: NSTableView
    private let scrollView: NSScrollView
    private let popupPanel: NSPanel
    private var filtered: [SearchResult] = []

    init(commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution, editor: Editor, onDismiss: @escaping () -> Void) {
        self.commit = commit
        self.expectedType = expectedType
        self.substitution = substitution
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

    // Tab / Shift-Tab handling.
    //
    // We handle tab advancement ourselves instead of letting NSTextField's built-in
    // Tab handling run, because the default path fights our dismiss-then-show pattern:
    //
    //   Default path:
    //     1. NSTextField calls endEditing with NSTabTextMovement
    //     2. controlTextDidEndEditing fires
    //     3. NSTextField then calls window.selectKeyView(following:/preceding:)
    //
    //   If our dismiss runs in step 2, the SearchPopup (containing the SearchField
    //   that's the current first responder) gets removeFromSuperview'd. That triggers
    //   NSControl._setWindow → abortEditing → makeFirstResponder(window), which
    //   clobbers focus. Step 3 then advances from whatever that clobber set — not
    //   from the SearchField's original position in the key view loop.
    //
    //   Previous workaround: defer dismiss via DispatchQueue.main.async so step 3
    //   runs while SearchField is still in place. This worked for focus but caused
    //   a distinct bug — during the async gap, the NEXT placeholder's becomeFirst-
    //   Responder fires and its popup anchors against the still-wide layout (the
    //   dismissing placeholder hasn't shrunk back to pill size yet). Result: the
    //   next popup's panel shows up at a stale screen X, often visibly too far right.
    //   See git history around this comment for the log that traced this.
    //
    //   Also tried: keyDown intercept on SearchField. Works but semantically dishonest
    //   ("a key was pressed" — we only care about one key). Handling in doCommandBy
    //   at the insertTab:/insertBacktab: selector is still slightly misnamed (we're
    //   not inserting tab characters), but it's Apple's own repurposed convention for
    //   single-line NSTextFields: the default insertTab: action on a single-line
    //   control calls endEditing with NSTabTextMovement, so we're replacing a
    //   dismiss-and-advance with our own dismiss-and-advance that orders correctly.
    //
    // Correct ordering, which this method implements:
    //   1. Capture nextValidKeyView while SearchField is still in the hierarchy
    //   2. onDismiss sync — SearchPopup removed, owning placeholder collapses to pill,
    //      layout settles
    //   3. makeFirstResponder on the captured target — its activate() anchors its
    //      popup against the settled (post-collapse) layout
    private func advanceKeyView(following: Bool) {
        let target = following ? searchField.nextValidKeyView : searchField.previousValidKeyView
        let window = self.window
        onDismiss()
        target.flatMap { window?.makeFirstResponder($0) }
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
        let entries = buildEntries(editor: editor, commit: commit, needle: needle, expectedType: expectedType, substitution: substitution)
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
        if commandSelector == #selector(NSResponder.insertTab(_:)) {
            advanceKeyView(following: true)
            return true
        }
        if commandSelector == #selector(NSResponder.insertBacktab(_:)) {
            advanceKeyView(following: false)
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
