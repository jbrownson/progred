import AppKit

class BracketedContainer: FlippedView {
    let bodyWrapper: NSView
    let closeLabel: NSTextField
    let openLabel: NSTextField
    let toggle: TriangleButton
    let openStr: String
    let closeStr: String

    init(open: String, close: String, body: NSView) {
        self.openStr = open
        self.closeStr = close
        self.toggle = TriangleButton(collapsed: false)
        self.openLabel = punctuationLabel(open)
        self.closeLabel = punctuationLabel(close)
        self.bodyWrapper = FlippedView()
        super.init(frame: .zero)

        bodyWrapper.addSubview(body)
        constrain(body, toFill: bodyWrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))

        let content = vStack([openLabel, bodyWrapper, closeLabel])
        let outer = hStack([toggle, content])
        addSubview(outer)
        constrain(outer, toFill: self)

        toggle.onToggle = { [weak self] collapsed in
            guard let self else { return }
            bodyWrapper.isHidden = collapsed
            closeLabel.isHidden = collapsed
            openLabel.stringValue = collapsed ? "\(openStr)…\(closeStr)" : openStr
        }
    }

    required init?(coder: NSCoder) { fatalError() }
}
