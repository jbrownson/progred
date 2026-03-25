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
        let resolved = reconcileChild(subviews.first, d, editor: editor)
        if resolved !== subviews.first {
            subviews.forEach { $0.removeFromSuperview() }
            addSubview(resolved)
            resolved.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                resolved.topAnchor.constraint(equalTo: topAnchor, constant: 8),
                resolved.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            ])
        }
    }

    override func layout() {
        super.layout()
        guard let clipView = superview as? NSClipView else { return }
        let visible = clipView.bounds.size
        let needed = subviews.reduce(CGSize.zero) { size, sub in
            CGSize(width: max(size.width, sub.frame.maxX + 8),
                   height: max(size.height, sub.frame.maxY + 8))
        }
        frame.size = NSSize(
            width: max(visible.width, needed.width),
            height: max(visible.height, needed.height))
    }

    override var acceptsFirstResponder: Bool { true }
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }
}

func createView(_ d: D, editor: Editor) -> NSView {
    switch d {
    case .text(let text, let style): DText(text, style)
    case .space: DSpace(spacing)
    case .identicon(let uuid): DIdenticon(uuid: uuid)
    case .block(let children): DBlock(children: children, editor: editor)
    case .line(let children): DLine(children: children, editor: editor)
    case .list(_, let elements): DList(elements: elements, editor: editor)
    case .indent(let child): DIndent(child: child, editor: editor)
    case .descend(let path, let child): DDescend(path: path, editor: editor, child: child)
    case .descendListElement(let consPath, let child): DListElement(consPath: consPath, editor: editor, child: child)
    case .collapse(let defaultCollapsed, let header, let body):
        DCollapse(defaultCollapsed: defaultCollapsed, header: header, body: body, editor: editor)
    case .bracketed(let open, let close, let body):
        DBracketed(open: open, close: close, body: body, editor: editor)
    case .placeholder: DText("_", .punctuation)
    case .stringEditor(let string): DText(string, .literal)
    case .numberEditor(let number): DText(String(number), .literal)
    }
}
