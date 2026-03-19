import AppKit

class TriangleButton: NSButton {
    var isCollapsed: Bool
    var onToggle: ((Bool) -> Void)?

    init(collapsed: Bool) {
        self.isCollapsed = collapsed
        super.init(frame: NSRect(x: 0, y: 0, width: 16, height: 16))
        isBordered = false
        title = ""
        imagePosition = .imageOnly
        target = self
        action = #selector(didPress)
        updateImage()
        setContentHuggingPriority(.required, for: .horizontal)
        setContentHuggingPriority(.required, for: .vertical)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 16).isActive = true
        heightAnchor.constraint(equalToConstant: 16).isActive = true
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc private func didPress() {
        isCollapsed.toggle()
        updateImage()
        onToggle?(isCollapsed)
    }

    private func updateImage() {
        let name = isCollapsed ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill"
        image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 7, weight: .regular))
        contentTintColor = .secondaryLabelColor
    }

    override var intrinsicContentSize: NSSize { NSSize(width: 16, height: 16) }
}
