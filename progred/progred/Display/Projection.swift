import AppKit

// Project-only for now. RootView.apply(delta) does a full rebuild;
// per-projection update functions will land later.

func projectValue(_ editor: Editor, _ ancestors: Set<UUID>, _ entity: Id?) -> NSView {
    guard let entity else { return Text("·", .literal) }

    if case .uuid(let uuid) = entity, ancestors.contains(uuid) {
        return Text("↻ \(editor.name(of: entity) ?? "<cycle>")", .keyword)
    }

    if case .string(let s) = entity { return Text(s, .literal) }
    if case .number(let n) = entity { return Text("\(n)", .literal) }

    guard case .uuid(let uuid) = entity else {
        return Text(editor.name(of: entity) ?? "\(entity)", .literal)
    }

    let recordType = editor.gid.get(entity: entity, label: editor.schema.recordField)
    if recordType == editor.schema.consRecord || recordType == editor.schema.emptyRecord {
        return projectList(editor, ancestors, uuid)
    }
    return projectRecord(editor, ancestors, uuid)
}

func projectRecord(_ editor: Editor, _ ancestors: Set<UUID>, _ entity: UUID) -> Block {
    let recordType = editor.gid.get(entity: .uuid(entity), label: editor.schema.recordField)
    let typeName = recordType.flatMap { editor.name(of: $0) } ?? "?"
    let header = Text(typeName, .typeRef)

    let fieldIds: [Id] = recordType.flatMap { type -> [Id]? in
        guard let fieldsListId = editor.gid.get(entity: type, label: editor.schema.fieldsField),
              case .uuid(let listUuid) = fieldsListId else { return nil }
        return walkCons(editor, from: listUuid)
    } ?? []

    let childAncestors = ancestors.union([entity])
    let rows = fieldIds.map { projectField(editor, childAncestors, entity, $0) }
    return Block([header, Indent(Block(rows))])
}

func projectField(_ editor: Editor, _ ancestors: Set<UUID>, _ parent: UUID, _ field: Id) -> Block {
    let labelLine = Line([
        Text(editor.name(of: field) ?? "?", .label),
        Text("→", .punctuation),
    ])
    let valueId = editor.gid.get(entity: .uuid(parent), label: field)
    let valueView = projectValue(editor, ancestors, valueId)
    return Block([labelLine, Indent(valueView)])
}

func projectList(_ editor: Editor, _ ancestors: Set<UUID>, _ entity: UUID) -> Block {
    let elements = walkCons(editor, from: entity)
    let childAncestors = ancestors.union([entity])
    let body = Block(elements.map { projectValue(editor, childAncestors, $0) })
    return Block([
        Text("[", .punctuation),
        Indent(body),
        Text("]", .punctuation),
    ])
}

/// Stops at emptyRecord; bails on non-cons cells (resilient to malformed chains).
func walkCons(_ editor: Editor, from entity: UUID) -> [Id] {
    var result: [Id] = []
    var current: Id = .uuid(entity)
    var visited: Set<UUID> = []
    while case .uuid(let uuid) = current {
        guard !visited.contains(uuid) else { break }
        visited.insert(uuid)
        guard let recordType = editor.gid.get(entity: current, label: editor.schema.recordField) else { break }
        if recordType == editor.schema.emptyRecord { break }
        guard recordType == editor.schema.consRecord else { break }
        guard let head = editor.gid.get(entity: current, label: editor.schema.headField) else { break }
        result.append(head)
        guard let tail = editor.gid.get(entity: current, label: editor.schema.tailField) else { break }
        current = tail
    }
    return result
}
