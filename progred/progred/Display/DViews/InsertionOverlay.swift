import AppKit

private let zoneOverlap: CGFloat = 3
private let caretSize: CGFloat = 6
private let trackingKey = "zone"

private struct Zone {
    let rect: NSRect
    let caretPoint: NSPoint
    let view: InsertionPointView
    let vertical: Bool
}

private func findZones(in root: NSView) -> [Zone] {
    var zones: [Zone] = []
    findZonesRecursive(root, in: root, zones: &zones)
    return zones
}

private func findZonesRecursive(_ view: NSView, in root: NSView, zones: inout [Zone]) {
    if let ip = view as? InsertionPointView, let parent = ip.superview {
        let size = ip.vertical ? ip.frame.height : ip.frame.width
        guard size < 1 else { return }
        let crossExtent: CGFloat = 20
        let rectInParent = ip.vertical
            ? NSRect(x: ip.frame.midX - crossExtent / 2, y: ip.frame.midY - zoneOverlap,
                     width: crossExtent, height: zoneOverlap * 2)
            : NSRect(x: ip.frame.midX - zoneOverlap, y: 0,
                     width: zoneOverlap * 2, height: parent.bounds.height)
        let originInRoot = root.convert(ip.frame.origin, from: parent)
        zones.append(Zone(
            rect: root.convert(rectInParent, from: parent),
            caretPoint: originInRoot,
            view: ip, vertical: ip.vertical))
        return
    }
    for subview in view.subviews where !subview.isHiddenOrHasHiddenAncestor {
        findZonesRecursive(subview, in: root, zones: &zones)
    }
}

private class DrawingOverlay: FlippedView {
    var zones: [Zone] = []

    override func draw(_ dirtyRect: NSRect) {
        for zone in zones {
            if false {
                NSColor.systemGreen.withAlphaComponent(0.1).setFill()
                NSBezierPath(rect: zone.rect).fill()
            }

            guard zone.view.isHovered else { continue }
            let path = NSBezierPath()
            if zone.vertical {
                let x = zone.caretPoint.x
                let midY = zone.rect.midY
                path.move(to: NSPoint(x: x - caretSize / 2, y: midY - caretSize / 2))
                path.line(to: NSPoint(x: x + caretSize / 2, y: midY))
                path.line(to: NSPoint(x: x - caretSize / 2, y: midY + caretSize / 2))
            } else {
                let midX = zone.caretPoint.x
                let bottom = zone.rect.maxY
                path.move(to: NSPoint(x: midX - caretSize / 2, y: bottom))
                path.line(to: NSPoint(x: midX, y: bottom - caretSize))
                path.line(to: NSPoint(x: midX + caretSize / 2, y: bottom))
            }
            NSColor.secondaryLabelColor.setStroke()
            path.lineWidth = 1.5
            path.stroke()
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

extension NSView {
    func rescanInsertionZones() {
        var view: NSView? = self
        while let v = view {
            if let overlay = v as? InsertionOverlay { overlay.rescan(); return }
            view = v.superview
        }
    }
}

class InsertionOverlay: FlippedView {
    private var zones: [(zone: Zone, tracking: NSTrackingArea)] = []
    private let drawingOverlay = DrawingOverlay()

    init() {
        super.init(frame: .zero)
        addSubview(drawingOverlay)
        constrain(drawingOverlay, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layout() {
        super.layout()
        updateZones()
    }

    func rescan() {
        layoutSubtreeIfNeeded()
        updateZones()
    }

    private func updateZones() {
        zones.forEach {
            $0.zone.view.isHovered = false
            removeTrackingArea($0.tracking)
        }
        let found = findZones(in: self)
        let mouseLocal = window.map { convert($0.mouseLocationOutsideOfEventStream, from: nil) }
        zones = found.map { zone in
            let area = NSTrackingArea(
                rect: zone.rect,
                options: [.mouseEnteredAndExited, .activeInKeyWindow],
                owner: self,
                userInfo: [trackingKey: zone.view])
            addTrackingArea(area)
            if let mouseLocal, zone.rect.contains(mouseLocal) { zone.view.isHovered = true }
            return (zone, area)
        }
        drawingOverlay.zones = found
        addSubview(drawingOverlay, positioned: .above, relativeTo: nil)
        drawingOverlay.needsDisplay = true
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let local = convert(point, from: superview)
        return zones.contains { $0.zone.rect.contains(local) } ? self : super.hitTest(point)
    }

    override func mouseDown(with event: NSEvent) {
        let local = convert(event.locationInWindow, from: nil)
        for (zone, _) in zones {
            if zone.rect.contains(local) { zone.view.activate(); return }
        }
        nextResponder?.mouseDown(with: event)
    }

    override func mouseEntered(with event: NSEvent) {
        guard let ip = event.trackingArea?.userInfo?[trackingKey] as? InsertionPointView else { return }
        ip.isHovered = true
        drawingOverlay.needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        guard let ip = event.trackingArea?.userInfo?[trackingKey] as? InsertionPointView else { return }
        ip.isHovered = false
        drawingOverlay.needsDisplay = true
    }
}
