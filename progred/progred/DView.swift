import SwiftUI

private struct SchemaKey: EnvironmentKey {
    static let defaultValue = Schema.bootstrap()
}

extension EnvironmentValues {
    var schema: Schema {
        get { self[SchemaKey.self] }
        set { self[SchemaKey.self] = newValue }
    }
}

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

        case .list(let separator, let elements):
            VStack(alignment: .leading, spacing: 2) {
                ForEach(elements.indices, id: \.self) { i in
                    HStack(spacing: 0) {
                        DView(d: elements[i])
                        if i < elements.count - 1 {
                            Text(separator).foregroundStyle(TextStyle.punctuation.color)
                        }
                    }
                }
            }

        case .entity(let uuid, let label, let ancestors):
            EntityDView(uuid: uuid, label: label, ancestors: ancestors)

        case .placeholder:
            Text("_").foregroundStyle(.tertiary)

        case .stringEditor(let s):
            Text(s).foregroundStyle(TextStyle.literal.color)

        case .numberEditor(let n):
            Text(String(n)).foregroundStyle(TextStyle.literal.color)
        }
    }

    @ViewBuilder
    private func lineView(_ children: [D]) -> some View {
        if let last = children.last,
           case .bracketed(let open, let close, let body) = last {
            let prefix = children.dropLast()
            if case .list(_, let elements) = body, elements.isEmpty {
                HStack(spacing: 4) {
                    ForEach(prefix.indices, id: \.self) { i in
                        DView(d: prefix[i])
                    }
                    Text("\(open)\(close)").foregroundStyle(TextStyle.punctuation.color)
                }
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        ForEach(prefix.indices, id: \.self) { i in
                            DView(d: prefix[i])
                        }
                        Text(open).foregroundStyle(TextStyle.punctuation.color)
                    }
                    DView(d: body).padding(.leading, 16)
                    Text(close).foregroundStyle(TextStyle.punctuation.color)
                }
            }
        } else {
            HStack(spacing: 4) {
                ForEach(children.indices, id: \.self) { i in
                    DView(d: children[i])
                }
            }
        }
    }

    @ViewBuilder
    private func bracketedView(open: String, close: String, body: D) -> some View {
        if case .list(_, let elements) = body, elements.isEmpty {
            Text("\(open)\(close)").foregroundStyle(TextStyle.punctuation.color)
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text(open).foregroundStyle(TextStyle.punctuation.color)
                DView(d: body).padding(.leading, 16)
                Text(close).foregroundStyle(TextStyle.punctuation.color)
            }
        }
    }
}

struct EntityDView: View {
    let uuid: UUID
    let label: String?
    let ancestors: Set<UUID>
    @Environment(\.schema) private var schema

    var body: some View {
        DView(d: project(entity: uuid, schema: schema, ancestors: ancestors, label: label))
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
                Button { isCollapsed.toggle() } label: {
                    Image(systemName: isCollapsed ? "chevron.right" : "chevron.down")
                        .font(.caption2)
                        .frame(width: 16, height: 16)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            if !isCollapsed {
                DView(d: bodyD).padding(.leading, 16)
            }
        }
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
