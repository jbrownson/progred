import SwiftUI

private enum Layout {
    static let indent: CGFloat = 16
}

struct DView: View {
    let d: D
    var focus: FocusState<FocusTarget?>.Binding
    var descendPath: Path? = nil

    private func withDescend<V: View>(_ view: V) -> some View {
        view.applyIf(descendPath) { v, path in
            v.modifier(DescendModifier(path: path, focus: focus))
        }
    }

    @ViewBuilder
    var body: some View {
        switch d {
        case .block(let children):
            withDescend(VStack(alignment: .leading, spacing: 0) {
                ForEach(children.indices, id: \.self) { i in
                    DView(d: children[i], focus: focus)
                }
            })

        case .line(let children):
            withDescend(HStack(spacing: 0) {
                ForEach(children.indices, id: \.self) { i in
                    DView(d: children[i], focus: focus)
                }
            })

        case .space:
            let spacing: CGFloat = 4
            withDescend(Spacer().frame(width: spacing, height: spacing))

        case .indent(let child):
            withDescend(DView(d: child, focus: focus).padding(.leading, Layout.indent))

        case .bracketed(let open, let close, let body):
            BracketedView(open: open, close: close, content: body, focus: focus, descendPath: descendPath)

        case .text(let s, let style):
            withDescend(Text(s).foregroundStyle(style.color))

        case .identicon(let uuid):
            withDescend(Identicon(uuid: uuid))

        case .descend(let path, let child):
            withDescend(DView(d: child, focus: focus, descendPath: path))

        case .collapse(let defaultCollapsed, let header, let body):
            CollapseView(defaultCollapsed: defaultCollapsed, header: header, body: body, focus: focus, descendPath: descendPath)

        case .list(_, let elements):
            withDescend(VStack(alignment: .leading, spacing: 0) {
                ForEach(elements.indices, id: \.self) { i in
                    DView(d: elements[i], focus: focus)
                }
            })

        case .placeholder:
            withDescend(Text("_").foregroundStyle(.tertiary))

        case .stringEditor(let s):
            withDescend(Text(s).foregroundStyle(TextStyle.literal.color))

        case .numberEditor(let n):
            withDescend(Text(String(n)).foregroundStyle(TextStyle.literal.color))
        }
    }
}

struct DescendModifier: ViewModifier {
    let path: Path
    var focus: FocusState<FocusTarget?>.Binding

    private var target: FocusTarget { FocusTarget(path: path, isCollapse: false) }

    func body(content: Content) -> some View {
        content
            .background {
                if focus.wrappedValue == target {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(.selection.opacity(0.3))
                        .padding(-2)
                }
            }
            .contentShape(Rectangle())
            .focusable()
            .focused(focus, equals: target)
            .focusEffectDisabled()
    }
}

struct CollapseView: View {
    let header: D
    let content: D
    var focus: FocusState<FocusTarget?>.Binding
    var descendPath: Path?
    @State private var isCollapsed = false

    init(defaultCollapsed: Bool = false, header: D, body: D, focus: FocusState<FocusTarget?>.Binding, descendPath: Path? = nil) {
        self._isCollapsed = State(initialValue: defaultCollapsed)
        self.header = header
        self.content = body
        self.focus = focus
        self.descendPath = descendPath
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 0) {
                DView(d: header, focus: focus)
                CollapseToggle(isCollapsed: $isCollapsed)
            }
            if !isCollapsed {
                DView(d: content, focus: focus).padding(.leading, Layout.indent)
            }
        }
        .applyIf(descendPath) { view, path in
            view.modifier(DescendModifier(path: path, focus: focus))
        }
    }
}

struct BracketedView: View {
    let open: String
    let close: String
    let content: D
    var focus: FocusState<FocusTarget?>.Binding
    var descendPath: Path?
    @State private var isCollapsed = false

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            CollapseToggle(isCollapsed: $isCollapsed)
            VStack(alignment: .leading, spacing: 0) {
                Text(isCollapsed ? "\(open)…\(close)" : open)
                    .foregroundStyle(TextStyle.punctuation.color)
                if !isCollapsed {
                    DView(d: content, focus: focus).padding(.leading, Layout.indent)
                    Text(close).foregroundStyle(TextStyle.punctuation.color)
                }
            }
            .applyIf(descendPath) { view, path in
                view.modifier(DescendModifier(path: path, focus: focus))
            }
        }
    }
}

// Wraps SwiftUI content in an NSView that absorbs mouseDown without
// forwarding to the responder chain. Prevents parent .focusable() views
// from claiming focus when this area is clicked. SwiftUI gesture
// recognizers inside still work — they receive events separately from
// the responder chain.
struct FocusShield<Content: View>: NSViewRepresentable {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    func updateNSView(_ nsView: NSView_, context: Context) {
        (nsView.subviews.first as? NSHostingView<Content>)?.rootView = content
    }

    class NSView_: NSView {
        override func mouseDown(with event: NSEvent) {}
    }

    static func makeShieldView(content: Content) -> NSView_ {
        let shield = NSView_()
        let hosting = NSHostingView(rootView: content)
        hosting.translatesAutoresizingMaskIntoConstraints = false
        shield.addSubview(hosting)
        NSLayoutConstraint.activate([
            hosting.leadingAnchor.constraint(equalTo: shield.leadingAnchor),
            hosting.trailingAnchor.constraint(equalTo: shield.trailingAnchor),
            hosting.topAnchor.constraint(equalTo: shield.topAnchor),
            hosting.bottomAnchor.constraint(equalTo: shield.bottomAnchor),
        ])
        return shield
    }

    func makeNSView(context: Context) -> NSView_ {
        Self.makeShieldView(content: content)
    }
}

struct CollapseToggle: View {
    @Binding var isCollapsed: Bool
    @FocusState private var isFocused: Bool
    @State private var isHovered = false

    var body: some View {
        let toggleSize: CGFloat = 16
        FocusShield {
            Button { isCollapsed.toggle() } label: {
                Image(systemName: isCollapsed ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill")
                    .font(.system(size: 7))
                    .foregroundStyle(isHovered ? .primary : .secondary)
                    .frame(width: toggleSize, height: toggleSize)
                    .background(
                        isHovered
                            ? AnyShapeStyle(.quaternary)
                            : AnyShapeStyle(.clear),
                        in: RoundedRectangle(cornerRadius: 3)
                    )
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .focusable(interactions: .activate)
            .focused($isFocused)
        }
        .onHover { isHovered = $0 }
    }
}

extension View {
    @ViewBuilder
    func applyIf<T, V: View>(_ value: T?, transform: (Self, T) -> V) -> some View {
        if let value { transform(self, value) } else { self }
    }
}

extension TextStyle {
    var color: Color {
        switch self {
        case .keyword: .purple
        case .typeRef: .cyan
        case .punctuation: .secondary
        case .label: .secondary
        case .literal: .primary
        }
    }
}

struct Identicon: View {
    let uuid: UUID

    var body: some View {
        Canvas { context, size in
            let u = uuid.uuid
            let bits = UInt16(u.0) | (UInt16(u.1) << 8)
            let color = Color(
                hue: Double(u.2) / 255.0,
                saturation: 0.5 + Double(u.3) / 255.0 * 0.3,
                brightness: 0.6 + Double(u.4) / 255.0 * 0.2
            )
            let cell = size.width / 5
            for row in 0..<5 {
                for col in 0..<3 {
                    if bits & (1 << (row * 3 + col)) != 0 {
                        let rect = CGRect(x: CGFloat(col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                        context.fill(SwiftUI.Path(rect), with: .color(color))
                        if col < 2 {
                            let mirror = CGRect(x: CGFloat(4 - col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                            context.fill(SwiftUI.Path(mirror), with: .color(color))
                        }
                    }
                }
            }
        }
        .frame(width: 10, height: 10)
    }
}
