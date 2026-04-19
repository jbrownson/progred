import AppKit

final class Block: NSStackView {
    let focusable: Bool
    override var isFlipped: Bool { true }

    init(_ views: [NSView] = [], focusable: Bool = true) {
        self.focusable = focusable
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        views.forEach { addArrangedSubview($0) }
    }
    required init?(coder: NSCoder) { fatalError() }

    override var acceptsFirstResponder: Bool { focusable }

    override func mouseDown(with event: NSEvent) {
        if focusable {
            window?.makeFirstResponder(self)
        } else {
            nextResponder?.mouseDown(with: event)
        }
    }
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok { setFocusIndicator(true) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { setFocusIndicator(false) }
        return ok
    }
}
