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

private func insertionPoint(tail: Id, link: @escaping Commit, expectedType: Id?, ctx: ProjectionContext) -> D {
    .insertionPoint(commit: { editor, id in
        let cons = UUID()
        editor.commit(entity: cons, label: ctx.recordField, value: ctx.consRecord)
        editor.commit(entity: cons, label: ctx.headField, value: id)
        editor.commit(entity: cons, label: ctx.tailField, value: tail)
        link(editor, .uuid(cons))
    }, expectedType: expectedType)
}

private func renderEmptyList(open: String, close: String, list: Id, expectedType: Id?, ctx: ProjectionContext) -> D {
    let items: [D] = ctx.commit.map { commit in
        [insertionPoint(tail: list, link: commit, expectedType: expectedType, ctx: ctx)]
    } ?? []
    return inlineBrackets(open: open, close: close, items)
}

private func mapBetween<T, U>(_ posts: [T], _ post: (T) -> U, _ panel: (T, T) -> U) -> [U] {
    guard let first = posts.first else { return [] }
    return [post(first)] + zip(posts, posts.dropFirst()).flatMap { [panel($0, $1), post($1)] }
}

func renderList(open: String = "[", close: String = "]", inline: Bool = false, elementRender: Render? = nil) -> Render {
    { ctx in
        guard let entity = ctx.entity,
              let (conses, empty, consesReadOnly) = ctx.conses(entity)
        else { return nil }

        let elementType = ctx.resolveExpectedType(for: ctx.headField)

        if conses.isEmpty {
            return renderEmptyList(open: open, close: close, list: entity, expectedType: elementType, ctx: ctx)
        }

        let listCommit: Commit? = consesReadOnly ? nil : ctx.commit

        let projectElement = { (cons: Id, prev: Id?) -> D in
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
            let consCtx = ctx.with(entity: cons, commit: listCommit != nil ? elementCommit : nil)
            return consCtx.descend(ctx.headField, render: elementRender, commit: elementCommit)
        }

        let consesWithPrev = zip(conses, [nil] + conses.dropLast().map(Optional.some))
            .map { ($0, $1) }

        let linkCommit = { (prev: Id) -> Commit in
            { editor, id in editor.commit(entity: prev.asUUID!, label: ctx.tailField, value: id) }
        }

        let items: [D] = listCommit.map { listCommit in
            [insertionPoint(tail: conses[0], link: listCommit, expectedType: elementType, ctx: ctx)]
                + mapBetween(consesWithPrev,
                    { projectElement($0.0, $0.1) },
                    { insertionPoint(tail: $1.0, link: linkCommit($0.0), expectedType: elementType, ctx: ctx) })
                + [insertionPoint(tail: empty, link: linkCommit(conses.last!), expectedType: elementType, ctx: ctx)]
        } ?? consesWithPrev.map { projectElement($0.0, $0.1) }

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

