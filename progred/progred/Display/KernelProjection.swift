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
              let (conses, consesReadOnly) = ctx.conses(entity)
        else { return nil }

        if conses.isEmpty {
            return renderEmptyList(open: open, close: close, list: entity, ctx: ctx)
        }

        let listCommit: Commit? = consesReadOnly ? nil : ctx.commit

        let prevConses = [nil] + conses.dropLast().map(Optional.some)
        let items = zip(conses, prevConses).map { cons, prev in
            let elementCommit: Commit = { editor, id in
                if let id {
                    editor.commit(entity: cons.asUUID!, label: ctx.headField, value: id)
                } else {
                    let tail = editor.gid.get(entity: cons, label: ctx.tailField)
                    if let prev {
                        editor.commit(entity: prev.asUUID!, label: ctx.tailField, value: tail)
                    } else {
                        listCommit!(editor, tail) // safe: elementCommit is only reachable when listCommit is non-nil
                    }
                }
            }

            let commit: Commit? = listCommit != nil ? elementCommit : nil
            let consCtx = ctx.with(entity: cons, commit: commit)
            return consCtx.descend(ctx.headField, render: elementRender, commit: commit)
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

