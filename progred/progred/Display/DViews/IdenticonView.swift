import AppKit

class IdenticonView: NSView {
    let uuid: UUID

    init(uuid: UUID) {
        self.uuid = uuid
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 10).isActive = true
        heightAnchor.constraint(equalToConstant: 10).isActive = true
    }

    required init?(coder: NSCoder) { fatalError() }
    override var intrinsicContentSize: NSSize { NSSize(width: 10, height: 10) }

    override func draw(_ dirtyRect: NSRect) {
        let u = uuid.uuid
        let bits = UInt16(u.0) | (UInt16(u.1) << 8)
        let color = NSColor(
            hue: CGFloat(u.2) / 255.0,
            saturation: 0.5 + CGFloat(u.3) / 255.0 * 0.3,
            brightness: 0.6 + CGFloat(u.4) / 255.0 * 0.2,
            alpha: 1)
        let cell = bounds.width / 5
        for row in 0..<5 {
            for col in 0..<3 {
                guard bits & (1 << (row * 3 + col)) != 0 else { continue }
                let rect = CGRect(x: CGFloat(col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                color.setFill()
                rect.fill()
                if col < 2 {
                    let mirror = CGRect(x: CGFloat(4 - col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                    mirror.fill()
                }
            }
        }
    }
}
