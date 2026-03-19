import AppKit

class TriangleButton: NSButton {
    var isCollapsed: Bool
    var onToggle: ((Bool) -> Void)?
    private var isHovered = false
    private var trackingArea: NSTrackingArea?

    init(collapsed: Bool) {
        self.isCollapsed = collapsed
        super.init(frame: NSRect(x: 0, y: 0, width: 16, height: 16))
        isBordered = false
        title = ""
        imagePosition = .imageOnly
        target = self
        action = #selector(didPress)
        updateAppearance()
        setContentHuggingPriority(.required, for: .horizontal)
        setContentHuggingPriority(.required, for: .vertical)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 16).isActive = true
        heightAnchor.constraint(equalToConstant: 16).isActive = true
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc private func didPress() {
        isCollapsed.toggle()
        updateAppearance()
        onToggle?(isCollapsed)
    }

    private func updateAppearance() {
        let name = isCollapsed ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill"
        image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 7, weight: .regular))
        contentTintColor = isHovered ? .labelColor : .secondaryLabelColor
    }

    override var intrinsicContentSize: NSSize { NSSize(width: 16, height: 16) }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInActiveApp], owner: self)
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
        updateAppearance()
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
        updateAppearance()
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        if isHovered {
            NSColor.quaternaryLabelColor.setFill()
            NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
        }
        super.draw(dirtyRect)
    }
}
