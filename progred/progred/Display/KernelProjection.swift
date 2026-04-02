import Foundation

func kernelHeader(ctx: ProjectionContext) -> D {
    let parts: [D] = [
        ctx.record().map { ctx.name(of: $0).map { .text($0, .typeRef) } ?? .placeholder },
        ctx.name().map { .text($0, .literal) },
    ].compactMap { $0 }
    if parts.isEmpty {
        guard let entity = ctx.entity else { return .placeholder }
        return rawHeader(entity)
    }
    return parts.count == 1 ? parts[0] : .line([parts[0], .space, parts[1]])
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

private func renderEmptyList(open: String, close: String, list: Id, ctx: ProjectionContext) -> D {
    let insertCommit = ctx.commit.map { commit in
        { (editor: Editor, id: Id) in
            let cons = UUID()
            editor.commit(entity: cons, label: ctx.recordField, value: ctx.consRecord)
            editor.commit(entity: cons, label: ctx.headField, value: id)
            editor.commit(entity: cons, label: ctx.tailField, value: list)
            commit(editor, .uuid(cons))
        }
    }
    let items: [D] = insertCommit.map { [.insertionPoint($0)] } ?? []
    return inlineBrackets(open: open, close: close, items)
}

func renderList(open: String = "[", close: String = "]", inline: Bool = false, elementRender: Render? = nil) -> Render {
    { ctx in
        guard let entity = ctx.entity,
              let elements = ctx.listToArray(entity)
        else { return nil }

        if elements.isEmpty {
            return renderEmptyList(open: open, close: close, list: entity, ctx: ctx)
        }

        var items: [D] = []
        for (i, el) in elements.enumerated() {
            let edgeReadOnly = ctx.gid.edges(entity: el.cons)?.readOnly ?? false
            let consCtx = ctx.with(entity: el.cons, commit: (ctx.commit == nil || edgeReadOnly) ? nil : ctx.commit)

            guard case .descend(let d) = consCtx.descend(ctx.headField, render: elementRender)
            else { continue }
            let elementCommit: Commit? = d.commit.map { headCommit in
                let spliceCommit: Commit = i == 0
                    ? { editor, _ in ctx.commit?(editor, ctx.gid.get(entity: el.cons, label: ctx.tailField)) }
                    : { editor, _ in
                        guard case .uuid(let prevUuid) = elements[i - 1].cons,
                              let tail = editor.gid.get(entity: el.cons, label: ctx.tailField)
                        else { return }
                        editor.commit(entity: prevUuid, label: ctx.tailField, value: tail)
                    }
                return { editor, id in
                    if let id { headCommit(editor, id) } else { spliceCommit(editor, nil) }
                }
            }
            items.append(.descend(Descend(
                inCycle: d.inCycle,
                commit: elementCommit,
                body: d.body)))
        }

        return inline
            ? inlineBrackets(open: open, close: close, items)
            : .bracketed(open: open, close: close,
                body: .list(separator: ",", elements: items))
    }
}

func projectKernel(_ ctx: ProjectionContext) -> D? {
    guard let entity = ctx.entity, ctx.record() != nil else { return nil }

    let header = kernelHeader(ctx: ctx)

    guard let raw = ctx.gid.edges(entity: entity) else { return header }
    let edges = raw.data
        .filter { $0.key != ctx.nameField && $0.key != ctx.recordField }
        .sorted { $0.key < $1.key }

    if edges.isEmpty { return header }

    return .collapse(collapsed: false, header: header) {
        .block(edges.map { label, _ in
            labeled(label, ctx.descend(label), ctx: ctx)
        })
    }
}

