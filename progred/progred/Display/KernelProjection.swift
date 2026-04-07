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

func renderList(open: String = "[", close: String = "]", inline: Bool = false, elementRender: Render? = nil) -> Render {
    { ctx in
        guard let entity = ctx.entity,
              let (conses, empty, consesReadOnly) = ctx.conses(entity)
        else { return nil }

        let elementType = ctx.resolveExpectedType(for: ctx.headField)
        let listCommit: Commit? = consesReadOnly ? nil : ctx.commit

        let elements: [D] = zip(conses, [nil] + conses.dropLast().map(Optional.some))
            .map { cons, prev in
                let elementCommit: Commit = { editor, id in
                    if let id {
                        editor.commit(entity: cons.asUUID!, label: ctx.headField, value: id)
                    } else {
                        let tail = editor.gid.get(entity: cons, label: ctx.tailField)
                        if let prev {
                            editor.commit(entity: prev.asUUID!, label: ctx.tailField, value: tail)
                        } else {
                            listCommit!(editor, tail)
                        }
                    }
                }
                let consCtx = ctx.with(entity: cons, commit: listCommit != nil ? elementCommit : nil)
                return consCtx.descend(ctx.headField, render: elementRender, commit: elementCommit)
            }

        let insertion: ListInsert? = listCommit.map { listCommit in
            ListInsert(
                insert: { editor, value, position in
                    let tail = position < conses.count ? conses[position] : empty
                    let link: Commit = position == 0
                        ? listCommit
                        : { editor, id in editor.commit(entity: conses[position - 1].asUUID!, label: ctx.tailField, value: id) }
                    let newCons = UUID()
                    editor.commit(entity: newCons, label: ctx.recordField, value: ctx.consRecord)
                    editor.commit(entity: newCons, label: ctx.headField, value: value)
                    editor.commit(entity: newCons, label: ctx.tailField, value: tail)
                    link(editor, .uuid(newCons))
                },
                expectedType: elementType,
                substitution: ctx.substitution)
        }

        return .list(List(
            open: open, close: close, separator: ", ",
            inline: inline,
            elements: elements,
            insertion: insertion))
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

