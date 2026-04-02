import Foundation

func spliceAction(consPath: Path) -> (Editor) -> Void {
    { editor in
        guard case .uuid(let consUuid) = consPath.node(in: editor.gid, root: editor.root),
              let tail = editor.gid.get(entity: .uuid(consUuid), label: editor.schema.tailField),
              let (parentPath, edgeLabel) = consPath.pop(),
              case .uuid(let parentUuid) = parentPath.node(in: editor.gid, root: editor.root)
        else { return }
        editor.commit(entity: parentUuid, label: edgeLabel, value: tail)
    }
}

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
              let elements = ctx.listToArray(entity)
        else { return nil }

        if elements.isEmpty {
            let insertCommit = ctx.commit.map { commit in
                { (editor: Editor, id: Id) in
                    let cons = UUID()
                    editor.commit(entity: cons, label: ctx.recordField, value: ctx.consRecord)
                    editor.commit(entity: cons, label: ctx.headField, value: id)
                    editor.commit(entity: cons, label: ctx.tailField, value: entity)
                    commit(editor, .uuid(cons))
                }
            }
            let items: [D] = insertCommit.map { [.insertionPoint($0)] } ?? []
            return inlineBrackets(open: open, close: close, items)
        }

        var consPath = ctx.path
        var items: [D] = []
        for el in elements {
            let consCommit: Commit? = (ctx.commit == nil
                || (ctx.gid.edges(entity: el.cons)?.readOnly ?? false)) ? nil : ctx.commit
            let consCtx = ctx.with(entity: el.cons, path: consPath, commit: consCommit)

            if case .descend(let d) = consCtx.descend(ctx.headField, render: elementRender) {
                let currentConsPath = consPath
                let elementCommit: Commit? = consCommit == nil ? nil : { editor, id in
                    if let id {
                        editor.commit(path: d.path, value: id)
                    } else {
                        spliceAction(consPath: currentConsPath)(editor)
                    }
                }
                items.append(.descend(Descend(
                    path: d.path,
                    inCycle: d.inCycle,
                    commit: elementCommit,
                    body: d.body)))
            }

            let tailPath = consPath.child(ctx.tailField)
            if ctx.focus == tailPath {
                items.append(.descend(Descend(path: tailPath, inCycle: false, commit: nil, body: .placeholder)))
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

