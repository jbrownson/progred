import AppKit
import SwiftUI

let indentWidth: CGFloat = 16
private let spacing: CGFloat = 4

// MARK: - SwiftUI bridge

struct DRender: NSViewRepresentable {
    let d: D
    let editor: Editor

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.verticalScrollElasticity = .none
        scrollView.horizontalScrollElasticity = .none

        let root = DRootView(editor: editor)
        scrollView.documentView = root
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let root = scrollView.documentView as? DRootView else { return }
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
        let content = renderD(d, editor: editor)
        addSubview(content)
        content.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            content.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            content.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
        ])
    }

    override func layout() {
        super.layout()
        guard let clipView = superview as? NSClipView else { return }
        let visible = clipView.bounds.size
        // Content frame is resolved by Auto Layout after super.layout()
        let needed = subviews.reduce(CGSize.zero) { size, sub in
            CGSize(width: max(size.width, sub.frame.maxX + 8),
                   height: max(size.height, sub.frame.maxY + 8))
        }
        frame.size = NSSize(
            width: max(visible.width, needed.width),
            height: max(visible.height, needed.height))
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
        constrain(view, toFill: wrapper, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
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
