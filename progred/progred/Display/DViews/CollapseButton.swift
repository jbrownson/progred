import AppKit

class CollapseButton: NSButton {
    static let size: CGFloat = 16

    var isCollapsed: Bool
    var onCollapsedChanged: ((Bool) -> Void)?
    private var isHovered = false
    private var trackingArea: NSTrackingArea?

    init(collapsed: Bool) {
        self.isCollapsed = collapsed
        super.init(frame: .zero)
        isBordered = false
        title = ""
        imagePosition = .imageOnly
        target = self
        action = #selector(didPress)
        updateAppearance()
        setContentHuggingPriority(.required, for: .horizontal)
        setContentHuggingPriority(.required, for: .vertical)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: Self.size).isActive = true
        heightAnchor.constraint(equalToConstant: Self.size).isActive = true
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc private func didPress() {
        isCollapsed.toggle()
        updateAppearance()
        onCollapsedChanged?(isCollapsed)
    }

    private func updateAppearance() {
        let name = isCollapsed ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill"
        image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 7, weight: .regular))
        contentTintColor = isHovered ? .labelColor : .secondaryLabelColor
    }

    override var intrinsicContentSize: NSSize { NSSize(width: Self.size, height: Self.size) }

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

    override func keyDown(with event: NSEvent) {
        if event.characters == " " { performClick(nil) }
        else { super.keyDown(with: event) }
    }

    override func draw(_ dirtyRect: NSRect) {
        if isHovered {
            NSColor.quaternaryLabelColor.setFill()
            NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
        }
        super.draw(dirtyRect)
    }
}
