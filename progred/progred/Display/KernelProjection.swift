import Foundation

func kernelHeader(ctx: ProjectionContext) -> D {
    let parts: [D] = [
        ctx.schema.record(of: ctx.entity).map { ctx.schema.name(of: $0).map { .text($0, .typeRef) } ?? .placeholder },
        ctx.schema.name(of: ctx.entity).map { .text($0, .literal) },
    ].compactMap { $0 }
    if parts.isEmpty { return rawHeader(ctx.entity) }
    return parts.count == 1 ? parts[0] : .line([parts[0], .space, parts[1]])
}

func flattenList(_ ctx: ProjectionContext) -> [Id]? {
    guard let rec = ctx.schema.record(of: ctx.entity) else { return nil }
    guard rec == ctx.schema.consRecord || rec == ctx.schema.emptyRecord else { return nil }
    return ctx.schema.listToArray(ctx.entity)
}

func inlineBrackets(open: String, close: String, _ items: [D]) -> D {
    var parts: [D] = [.text(open, .punctuation)]
    for (i, item) in items.enumerated() {
        if i > 0 { parts.append(.text(",", .punctuation)) }
        parts.append(item)
    }
    parts.append(.text(close, .punctuation))
    return .line(parts)
}

func renderList(open: String = "[", close: String = "]", inline: Bool = false, elementRender: Render? = nil) -> Render {
    { ctx in
        guard let elements = flattenList(ctx) else { return nil }
        let items = elements.map { el in
            D.selectable(SelectionActions(), child: ctx.descend(to: el, render: elementRender))
        }
        return elements.isEmpty || inline
            ? inlineBrackets(open: open, close: close, items)
            : .bracketed(open: open, close: close,
                body: .list(separator: ",", elements: items))
    }
}

func projectKernel(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) != nil else { return nil }

    let header = kernelHeader(ctx: ctx)

    guard let raw = ctx.schema.gid.edges(entity: ctx.entity) else { return header }
    let edges = raw
        .filter { $0.key != ctx.schema.nameField && $0.key != ctx.schema.recordField }
        .sorted { $0.key < $1.key }

    if edges.isEmpty { return header }

    let body: D = .block(edges.map { label, value in
        kernelEdge(label: label, value: value, ctx: ctx)
    })

    return .collapse(header: header, body: body)
}

private func kernelEdge(label: Id, value: Id, ctx: ProjectionContext) -> D {
    labeled(label, .selectable(SelectionActions(), child: ctx.descend(to: value)), schema: ctx.schema)
}
