import AppKit

/// Empty slot. Stub: just a small grey square. The pill + search popup
/// machinery from the old architecture will be rebuilt on top of this once
/// the rest of the projection layer is in place.
class PlaceholderView: NSView, Projection {
    init(_ ctx: ProjectionContext) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {}

    override var intrinsicContentSize: NSSize {
        let h = NSFont.systemFont(ofSize: NSFont.systemFontSize).boundingRectForFont.height
        return NSSize(width: ceil(h), height: ceil(h))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.separatorColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
    }
}
