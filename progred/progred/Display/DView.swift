import SwiftUI

struct DView: View {
    let d: D

    @ViewBuilder
    var body: some View {
        switch d {
        case .block(let children):
            LazyVStack(alignment: .leading, spacing: 2) {
                ForEach(children.indices, id: \.self) { i in
                    DView(d: children[i])
                }
            }

        case .line(let children):
            lineView(children)

        case .indent(let child):
            DView(d: child).padding(.leading, 16)

        case .bracketed(let open, let close, let body):
            bracketedView(open: open, close: close, body: body)

        case .text(let s, let style):
            Text(s).foregroundStyle(style.color)

        case .identicon(let uuid):
            Identicon(uuid: uuid)

        case .descend(_, let child):
            DView(d: child)

        case .collapse(let collapsed, let label, let body):
            CollapseView(collapsed: collapsed, label: label, body: body)

        case .list(_, let elements):
            VStack(alignment: .leading, spacing: 2) {
                ForEach(elements.indices, id: \.self) { i in
                    DView(d: elements[i])
                }
            }

        case .placeholder:
            Text("_").foregroundStyle(.tertiary)

        case .stringEditor(let s):
            Text(s).foregroundStyle(TextStyle.literal.color)

        case .numberEditor(let n):
            Text(String(n)).foregroundStyle(TextStyle.literal.color)
        }
    }

    private func isInline(_ d: D) -> Bool {
        switch d {
        case .text, .identicon, .placeholder, .stringEditor, .numberEditor: true
        case .descend(_, let child): isInline(child)
        case .line: true
        default: false
        }
    }

    private func unwrap(_ d: D) -> D {
        if case .descend(_, let child) = d { return unwrap(child) }
        return d
    }

    @ViewBuilder
    private func lineView(_ children: [D]) -> some View {
        if let splitIdx = children.firstIndex(where: { !isInline($0) }),
           case .bracketed(let open, let close, let body) = unwrap(children[splitIdx]) {
            BracketedLineView(
                prefix: Array(children[..<splitIdx]),
                open: open, close: close, content: body
            )
        } else {
            HStack(spacing: 4) {
                ForEach(children.indices, id: \.self) { i in DView(d: children[i]) }
            }
        }
    }

    @ViewBuilder
    private func bracketedView(open: String, close: String, body: D) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(open).foregroundStyle(TextStyle.punctuation.color)
            DView(d: body).padding(.leading, 16)
            Text(close).foregroundStyle(TextStyle.punctuation.color)
        }
    }
}

struct CollapseView: View {
    let label: D
    let bodyD: D
    @State private var isCollapsed: Bool

    init(collapsed: Bool, label: D, body: D) {
        self._isCollapsed = State(initialValue: collapsed)
        self.label = label
        self.bodyD = body
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                DView(d: label)
                CollapseToggle(isCollapsed: $isCollapsed)
            }
            if !isCollapsed {
                DView(d: bodyD).padding(.leading, 16)
            }
        }
    }
}

struct BracketedLineView: View {
    let prefix: [D]
    let open: String
    let close: String
    let content: D
    @State private var isCollapsed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                ForEach(prefix.indices, id: \.self) { i in DView(d: prefix[i]) }
                CollapseToggle(isCollapsed: $isCollapsed)
                Text(isCollapsed ? "\(open)\(close)" : open)
                    .foregroundStyle(TextStyle.punctuation.color)
            }
            if !isCollapsed {
                DView(d: content).padding(.leading, 16)
                Text(close).foregroundStyle(TextStyle.punctuation.color)
            }
        }
    }
}

struct CollapseToggle: View {
    @Binding var isCollapsed: Bool
    @State private var isHovered = false

    var body: some View {
        Button { isCollapsed.toggle() } label: {
            Image(systemName: isCollapsed ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill")
                .font(.system(size: 7))
                .foregroundStyle(isHovered ? .primary : .secondary)
                .frame(width: 16, height: 16)
                .background(
                    isHovered
                        ? AnyShapeStyle(.quaternary)
                        : AnyShapeStyle(.clear),
                    in: RoundedRectangle(cornerRadius: 3)
                )
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
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
                        context.fill(Path(rect), with: .color(color))
                        if col < 2 {
                            let mirror = CGRect(x: CGFloat(4 - col) * cell, y: CGFloat(row) * cell, width: cell, height: cell)
                            context.fill(Path(mirror), with: .color(color))
                        }
                    }
                }
            }
        }
        .frame(width: 10, height: 10)
    }
}
