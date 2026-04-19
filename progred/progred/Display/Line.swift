import AppKit

final class Line: NSStackView {
    override var isFlipped: Bool { true }

    init(_ views: [NSView] = []) {
        super.init(frame: .zero)
        orientation = .horizontal
        alignment = .firstBaseline
        spacing = 4
        translatesAutoresizingMaskIntoConstraints = false
        views.forEach { addArrangedSubview($0) }
    }
    required init?(coder: NSCoder) { fatalError() }
}
