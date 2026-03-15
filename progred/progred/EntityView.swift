import SwiftUI

struct EntityView: View {
    let entity: UUID
    let schema: Schema
    let label: String?
    let expanded: Bool

    init(entity: UUID, schema: Schema, label: String? = nil, expanded: Bool = false) {
        self.entity = entity
        self.schema = schema
        self.label = label
        self.expanded = expanded
    }

    private var header: Text {
        var result = Text("")
        if let label {
            result = Text("\(result)\(Text("\(label): ").foregroundStyle(.secondary))")
        }
        if let recId = schema.record(of: entity),
           let recName = schema.name(of: recId) {
            result = Text("\(result)\(Text("\(recName) ").foregroundStyle(.cyan))")
        }
        if let name = schema.name(of: entity) {
            result = Text("\(result)\(name)")
        }
        if schema.record(of: entity) == nil && schema.name(of: entity) == nil {
            result = Text("\(result)\(String(entity.uuidString.prefix(8)))")
        }
        return result
    }

    private var edges: [LabeledEdge] {
        guard let raw = schema.gid.edges(entity: .uuid(entity)) else { return [] }
        return raw
            .filter { $0.key != .uuid(schema.nameField) && $0.key != .uuid(schema.recordField) }
            .sorted { $0.key < $1.key }
            .map { LabeledEdge(parent: entity, label: $0.key, value: $0.value) }
    }

    private var hasName: Bool {
        schema.name(of: entity) != nil || schema.record(of: entity).flatMap { schema.name(of: $0) } != nil
    }

    private var headerLabel: some View {
        HStack(spacing: 4) {
            if !hasName {
                Identicon(uuid: entity)
            }
            header
        }
    }

    var body: some View {
        let edges = self.edges
        if edges.isEmpty {
            TreeLeaf { headerLabel }
        } else {
            TreeNode(expanded: expanded, label: { headerLabel }) {
                ForEach(edges) { edge in
                    EdgeValueView(label: edge.label, value: edge.value, schema: schema)
                }
            }
        }
    }
}

struct LabeledEdge: Identifiable {
    let parent: UUID
    let label: Id
    let value: Id
    var id: EdgeId { EdgeId(parent: parent, label: label) }
}

struct EdgeId: Hashable {
    let parent: UUID
    let label: Id
}

struct EdgeValueView: View {
    let label: Id
    let value: Id
    let schema: Schema

    private var labelName: String {
        label.asUUID.flatMap { schema.name(of: $0) } ?? "\(label)"
    }

    @ViewBuilder
    var body: some View {
        switch value {
        case .string(let s):
            TreeLeaf {
                HStack {
                    Text(labelName).foregroundStyle(.secondary)
                    Text("\"\(s)\"")
                }
            }
        case .number(let n):
            TreeLeaf {
                HStack {
                    Text(labelName).foregroundStyle(.secondary)
                    Text(String(n))
                }
            }
        case .uuid(let uuid):
            if isList(uuid) {
                listView(uuid)
            } else {
                EntityView(entity: uuid, schema: schema, label: labelName)
            }
        }
    }

    private func isList(_ uuid: UUID) -> Bool {
        guard let rec = schema.record(of: uuid) else { return false }
        return rec == schema.consRecord || rec == schema.emptyRecord
    }

    @ViewBuilder
    private func listView(_ head: UUID) -> some View {
        let elements = schema.listToArray(head)
        if elements.isEmpty {
            TreeLeaf {
                HStack {
                    Text(labelName).foregroundStyle(.secondary)
                    Text("(empty)").foregroundStyle(.tertiary)
                }
            }
        } else {
            TreeNode(label: {
                HStack {
                    Text(labelName).foregroundStyle(.secondary)
                    Text("(\(elements.count))").foregroundStyle(.tertiary)
                }
            }) {
                ForEach(Array(elements.enumerated()), id: \.element) { index, element in
                    EntityView(entity: element, schema: schema, label: "[\(index)]")
                }
            }
        }
    }
}

// MARK: - Tree layout

struct TreeNode<Label: View, Content: View>: View {
    @State private var isExpanded: Bool
    @ViewBuilder let label: Label
    @ViewBuilder let content: Content

    init(
        expanded: Bool = false,
        @ViewBuilder label: () -> Label,
        @ViewBuilder content: () -> Content
    ) {
        self._isExpanded = State(initialValue: expanded)
        self.label = label()
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                Button { isExpanded.toggle() } label: {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption2)
                        .frame(width: 10)
                }
                .buttonStyle(.plain)
                label
            }
            if isExpanded {
                VStack(alignment: .leading, spacing: 2) {
                    content
                }
                .padding(.leading, 16)
            }
        }
    }
}

struct TreeLeaf<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        HStack(spacing: 4) {
            Spacer().frame(width: 10)
            content
        }
    }
}

// MARK: - Identicon

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
