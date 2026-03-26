import Foundation

func kernelHeader(ctx: ProjectionContext) -> D {
    let parts: [D] = [
        ctx.record().map { ctx.name(of: $0).map { .text($0, .typeRef) } ?? .placeholder },
        ctx.name().map { .text($0, .literal) },
    ].compactMap { $0 }
    if parts.isEmpty { return rawHeader(ctx.entity) }
    return parts.count == 1 ? parts[0] : .line([parts[0], .space, parts[1]])
}

func flattenList(_ ctx: ProjectionContext) -> [ProjectionContext.ListElement]? {
    ctx.listToArray(ctx.entity)
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

        if elements.isEmpty {
            let insertPath = ctx.path.child(ctx.insertField)
            return ctx.focus == insertPath
                ? .descend(insertPath, readOnly: ctx.readOnly, child: .placeholder)
                : inlineBrackets(open: open, close: close, [])
        }

        var consPath = ctx.path
        var items: [D] = []
        for el in elements {
            let elementPath = consPath.child(ctx.headField)
            items.append(.descendListElement(consPath: consPath, readOnly: ctx.readOnly, child: ctx.descend(to: el.head, via: elementPath, render: elementRender).d))

            let tailPath = consPath.child(ctx.tailField)
            if ctx.focus == tailPath {
                items.append(.descend(tailPath, readOnly: ctx.readOnly, child: .placeholder))
            }

            consPath = consPath.child(ctx.tailField)
        }

        return inline
            ? inlineBrackets(open: open, close: close, items)
            : .bracketed(open: open, close: close,
                body: .list(separator: ",", elements: items))
    }
}

func projectKernel(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() != nil else { return nil }

    let header = kernelHeader(ctx: ctx)

    guard let raw = ctx.gid.edges(entity: ctx.entity) else { return header }
    let edges = raw.data
        .filter { $0.key != ctx.nameField && $0.key != ctx.recordField }
        .sorted { $0.key < $1.key }

    if edges.isEmpty { return header }

    let body: D = .block(edges.map { label, value in
        kernelEdge(label: label, value: value, ctx: ctx)
    })

    return .collapse(header: header, body: body)
}

private func kernelEdge(label: Id, value: Id, ctx: ProjectionContext) -> D {
    let childPath = ctx.path.child(label)
    return labeled(label, .descend(childPath, readOnly: ctx.readOnly, child: ctx.descend(to: value, via: childPath).d), ctx: ctx)
}
