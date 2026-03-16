import Foundation

func kernelHeader(ctx: ProjectionContext) -> D {
    var parts: [D] = []
    if let label = ctx.label {
        parts.append(.text(label, .label))
        parts.append(.text("→", .punctuation))
    }
    if let recName = ctx.record().flatMap({ ctx.schema.name(of: $0) }) {
        parts.append(.text(recName, .typeRef))
    }
    if let name = ctx.name() {
        parts.append(.text(name, .literal))
    }
    if ctx.record() == nil && ctx.name() == nil {
        parts.append(.identicon(ctx.entity))
    }
    return parts.count == 1 ? parts[0] : .line(parts)
}

func projectKernel(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() != nil || ctx.name() != nil else { return nil }

    let header = kernelHeader(ctx: ctx)

    guard let raw = ctx.schema.gid.edges(entity: .uuid(ctx.entity)) else { return header }
    let edges = raw
        .filter { $0.key != .uuid(ctx.schema.nameField) && $0.key != .uuid(ctx.schema.recordField) }
        .sorted { $0.key < $1.key }

    if edges.isEmpty { return header }

    let body: D = .block(edges.map { label, value in
        kernelEdge(label: label, value: value, ctx: ctx)
    })

    return .collapse(collapsed: false, label: header, body: body)
}

private func kernelEdge(label: Id, value: Id, ctx: ProjectionContext) -> D {
    let labelName = label.asUUID.flatMap { ctx.schema.name(of: $0) } ?? "\(label)"

    switch value {
    case .string(let s):
        return .line([.text(labelName, .label), .text("→", .punctuation), .text(s, .literal)])
    case .number(let n):
        return .line([.text(labelName, .label), .text("→", .punctuation), .text(String(n), .literal)])
    case .uuid(let uuid):
        if isList(uuid, schema: ctx.schema) {
            let elements = ctx.schema.listToArray(uuid)
            if elements.isEmpty {
                return .line([.text(labelName, .label), .text("→", .punctuation), .text("[]", .punctuation)])
            }
            return .line([
                .text(labelName, .label),
                .text("→", .punctuation),
                .bracketed(open: "[", close: "]",
                    body: .list(separator: ",", elements: elements.map { ctx.descend($0) })),
            ])
        } else {
            return .descend(label: label, child: ctx.descend(uuid, label: labelName))
        }
    }
}

private func isList(_ uuid: UUID, schema: Schema) -> Bool {
    guard let rec = schema.record(of: uuid) else { return false }
    return rec == schema.consRecord || rec == schema.emptyRecord
}
