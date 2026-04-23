import AppKit

final class Block: NSStackView {
    override var isFlipped: Bool { true }

    init(_ views: [NSView] = []) {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        views.forEach { addArrangedSubview($0) }
    }
    required init?(coder: NSCoder) { fatalError() }

    override func mouseDown(with event: NSEvent) {
        nextResponder?.mouseDown(with: event)
    }
}
