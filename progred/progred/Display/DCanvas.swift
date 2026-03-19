import AppKit
import SwiftUI

private let indentWidth: CGFloat = 16
private let spacing: CGFloat = 4

// MARK: - SwiftUI bridge

struct DCanvas: NSViewRepresentable {
    let d: D
    let editor: Editor

    func makeNSView(context: Context) -> DRootView {
        DRootView(editor: editor)
    }

    func updateNSView(_ root: DRootView, context: Context) {
        root.rebuild(d)
    }
}

// MARK: - Root

class DRootView: FlippedView {
    let editor: Editor

    init(editor: Editor) {
        self.editor = editor
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError() }

    func rebuild(_ d: D) {
        subviews.forEach { $0.removeFromSuperview() }
        let child = renderD(d, editor: editor)
        addSubview(child)
        pin(child, to: self, insets: NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8))
    }

    // Click background to deselect
    override var acceptsFirstResponder: Bool { true }
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }
}

// MARK: - Render

func renderD(_ d: D, editor: Editor) -> NSView {
    switch d {
    case .block(let children):
        return vStack(children.map { renderD($0, editor: editor) })

    case .line(let children):
        return hStack(children.map { renderD($0, editor: editor) })

    case .space:
        return spacer(spacing)

    case .indent(let child):
        let view = renderD(child, editor: editor)
        let wrapper = FlippedView()
        wrapper.addSubview(view)
        pin(view, to: wrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        return wrapper

    case .text(let s, let style):
        return label(s, color: style.nsColor)

    case .identicon(let uuid):
        return IdenticonView(uuid: uuid)

    case .descend(let path, let child):
        return SelectableView(path: path, editor: editor, child: renderD(child, editor: editor))

    case .collapse(let defaultCollapsed, let header, let body):
        return CollapseContainer(
            defaultCollapsed: defaultCollapsed,
            header: renderD(header, editor: editor),
            body: renderD(body, editor: editor))

    case .bracketed(let open, let close, let body):
        return BracketedContainer(
            open: open, close: close,
            body: renderD(body, editor: editor))

    case .list(_, let elements):
        return vStack(elements.map { renderD($0, editor: editor) })

    case .placeholder:
        return label("_", color: .tertiaryLabelColor)

    case .stringEditor(let s):
        return label(s, color: TextStyle.literal.nsColor)

    case .numberEditor(let n):
        return label(String(n), color: TextStyle.literal.nsColor)
    }
}

// MARK: - Selectable

class SelectableView: FlippedView {
    let path: Path
    weak var editor: Editor?

    init(path: Path, editor: Editor, child: NSView) {
        self.path = path
        self.editor = editor
        super.init(frame: .zero)
        addSubview(child)
        pin(child, to: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    override func resignFirstResponder() -> Bool {
        needsDisplay = true
        return true
    }

    private var isSelected: Bool {
        window?.firstResponder === self
    }

    override func draw(_ dirtyRect: NSRect) {
        guard isSelected else { return }
        NSColor.selectedContentBackgroundColor.withAlphaComponent(0.3).setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 3, yRadius: 3).fill()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        switch Int(event.keyCode) {
        case 51: editor?.handleDelete(path: path) // Delete
        case 53: window?.makeFirstResponder(superview) // Escape
        default: super.keyDown(with: event)
        }
    }
}

// MARK: - Collapse

class CollapseContainer: FlippedView {
    let bodyWrapper: NSView
    let toggle: TriangleButton

    init(defaultCollapsed: Bool, header: NSView, body: NSView) {
        self.toggle = TriangleButton(collapsed: defaultCollapsed)
        self.bodyWrapper = FlippedView()
        super.init(frame: .zero)

        let headerRow = hStack([toggle, header])

        bodyWrapper.addSubview(body)
        pin(body, to: bodyWrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        bodyWrapper.isHidden = defaultCollapsed

        let stack = vStack([headerRow, bodyWrapper])
        addSubview(stack)
        pin(stack, to: self)

        toggle.onToggle = { [weak self] collapsed in
            self?.bodyWrapper.isHidden = collapsed
        }
    }

    required init?(coder: NSCoder) { fatalError() }
}

// MARK: - Bracketed

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
        pin(body, to: bodyWrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))

        let content = vStack([openLabel, bodyWrapper, closeLabel])
        let outer = hStack([toggle, content])
        addSubview(outer)
        pin(outer, to: self)

        toggle.onToggle = { [weak self] collapsed in
            guard let self else { return }
            bodyWrapper.isHidden = collapsed
            closeLabel.isHidden = collapsed
            openLabel.stringValue = collapsed ? "\(openStr)…\(closeStr)" : openStr
        }
    }

    required init?(coder: NSCoder) { fatalError() }
}

// MARK: - Toggle button

class TriangleButton: NSButton {
    var isCollapsed: Bool
    var onToggle: ((Bool) -> Void)?

    init(collapsed: Bool) {
        self.isCollapsed = collapsed
        super.init(frame: NSRect(x: 0, y: 0, width: 16, height: 16))
        isBordered = false
        title = ""
        target = self
        action = #selector(didPress)
        setContentHuggingPriority(.required, for: .horizontal)
        setContentHuggingPriority(.required, for: .vertical)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 16).isActive = true
        heightAnchor.constraint(equalToConstant: 16).isActive = true
    }

    required init?(coder: NSCoder) { fatalError() }

    @objc private func didPress() {
        isCollapsed.toggle()
        needsDisplay = true
        onToggle?(isCollapsed)
    }

    override var intrinsicContentSize: NSSize { NSSize(width: 16, height: 16) }

    override func draw(_ dirtyRect: NSRect) {
        let path = NSBezierPath()
        let cx = bounds.midX, cy = bounds.midY
        let s: CGFloat = 3.5

        if isCollapsed {
            path.move(to: CGPoint(x: cx - s/2, y: cy - s))
            path.line(to: CGPoint(x: cx + s/2, y: cy))
            path.line(to: CGPoint(x: cx - s/2, y: cy + s))
        } else {
            path.move(to: CGPoint(x: cx - s, y: cy - s/2))
            path.line(to: CGPoint(x: cx, y: cy + s/2))
            path.line(to: CGPoint(x: cx + s, y: cy - s/2))
        }
        path.close()
        NSColor.secondaryLabelColor.setFill()
        path.fill()
    }
}

// MARK: - Identicon

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

// MARK: - Helpers

class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

private func label(_ text: String, color: NSColor) -> NSTextField {
    let field = NSTextField(labelWithString: text)
    field.textColor = color
    field.font = .systemFont(ofSize: NSFont.systemFontSize)
    field.translatesAutoresizingMaskIntoConstraints = false
    return field
}

private func punctuationLabel(_ text: String) -> NSTextField {
    label(text, color: TextStyle.punctuation.nsColor)
}

private func spacer(_ size: CGFloat) -> NSView {
    let view = NSView()
    view.translatesAutoresizingMaskIntoConstraints = false
    view.widthAnchor.constraint(equalToConstant: size).isActive = true
    view.heightAnchor.constraint(equalToConstant: size).isActive = true
    return view
}

private func vStack(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 0
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

private func hStack(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.alignment = .top
    stack.spacing = 0
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

private func pin(_ child: NSView, to parent: NSView, insets: NSEdgeInsets = NSEdgeInsets()) {
    child.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
        child.topAnchor.constraint(equalTo: parent.topAnchor, constant: insets.top),
        child.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: insets.left),
        child.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -insets.right),
        child.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -insets.bottom),
    ])
}

extension TextStyle {
    var nsColor: NSColor {
        switch self {
        case .keyword: .systemPurple
        case .typeRef: .systemCyan
        case .punctuation: .secondaryLabelColor
        case .label: .secondaryLabelColor
        case .literal: .labelColor
        }
    }
}
