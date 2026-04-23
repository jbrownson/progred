import AppKit
import HashTreeCollections

func projectId(_ editor: Editor, _ ancestors: Set<UUID>, _ id: Id?, _ commit: Commit?) -> NSView {
    guard let id else { return Text("·", .literal) }
    switch id {
    case .string(let s): return Text("\"\(s)\"", .literal)
    case .number(let n): return Text("\(n)", .literal)
    case .uuid(let uuid): return projectUUID(editor, ancestors, uuid, commit)
    }
}

func projectUUID(_ editor: Editor, _ ancestors: Set<UUID>, _ uuid: UUID, _ commit: Commit?) -> NSView {
    if ancestors.contains(uuid) {
        return Text("↻ \(editor.name(of: .uuid(uuid)) ?? "<cycle>")", .keyword)
    }
    return projectList(editor, ancestors, list: uuid, commit)
        ?? projectRecord(editor, ancestors, record: uuid)
        ?? projectRaw(editor, ancestors, entity: uuid)
}

func projectRecord(_ editor: Editor, _ ancestors: Set<UUID>, record: UUID) -> NSView? {
    guard let recordType = editor.gid.get(entity: .uuid(record), label: editor.schema.recordField),
          let fieldsListId = editor.gid.get(entity: recordType, label: editor.schema.fieldsField),
          case .uuid(let listUuid) = fieldsListId,
          let conses = conses(editor, of: listUuid)
    else { return nil }
    let typeName = editor.name(of: recordType) ?? "?"
    let header = Text(typeName, .typeRef)
    let childAncestors = ancestors.union([record])
    let rows = conses.map { projectField(editor, childAncestors, record, $0.head) }
    return Block([header, Indent(Block(rows))])
}

func projectField(_ editor: Editor, _ ancestors: Set<UUID>, _ parent: UUID, _ field: Id) -> NSView {
    let valueCommit: Commit = { newValue in
        editor.apply(GraphDelta.setting(entity: parent, label: field, value: newValue))
    }
    return Block([
        Line([Text(editor.name(of: field) ?? "?", .label), Text("→", .punctuation)]),
        Indent(Selectable(
            projectId(
                editor, ancestors,
                editor.gid.get(entity: .uuid(parent), label: field),
                valueCommit),
            commit: valueCommit))
    ])
}

func projectList(_ editor: Editor, _ ancestors: Set<UUID>, list: UUID, _ listCommit: Commit?) -> NSView? {
    let recordType = editor.gid.get(entity: .uuid(list), label: editor.schema.recordField)
    guard recordType == editor.schema.consRecord || recordType == editor.schema.emptyRecord,
          let conses = conses(editor, of: list)
    else { return nil }
    if conses.isEmpty {
        return Text("[]", .punctuation)
    }
    let childAncestors = ancestors.union([list])
    let elementViews = conses.enumerated().map { i, current -> Selectable in
        let cons = current.cons
        let prev: UUID? = i > 0 ? conses[i - 1].cons : nil
        let elementCommit: Commit = { newValue in
            if let newValue {
                editor.apply(GraphDelta.setting(entity: cons, label: editor.schema.headField, value: newValue))
            } else if let prev {
                editor.apply(spliceCons(editor, cons: cons, prev: prev))
            } else {
                listCommit?(editor.gid.get(entity: .uuid(cons), label: editor.schema.tailField))
            }
        }
        return Selectable(
            projectId(editor, childAncestors, current.head, elementCommit),
            commit: elementCommit)
    }
    let body = Block(elementViews)
    return Block([
        Text("[", .punctuation),
        Indent(body),
        Text("]", .punctuation),
    ])
}

func projectRaw(_ editor: Editor, _ ancestors: Set<UUID>, entity: UUID) -> NSView {
    let recordType = editor.gid.get(entity: .uuid(entity), label: editor.schema.recordField)
    let typeName = recordType.flatMap { editor.name(of: $0) } ?? "?"
    let header = Text(typeName, .typeRef)
    let headerLabels: Set<Id> = [editor.schema.recordField, editor.schema.nameField]
    let labels = (editor.gid.edges(entity: .uuid(entity))?.data ?? [:])
        .keys
        .filter { !headerLabels.contains($0) }
        .sorted()
    let childAncestors = ancestors.union([entity])
    let rows = labels.map { projectField(editor, childAncestors, entity, $0) }
    return Block([header, Indent(Block(rows))])
}

func spliceCons(_ editor: Editor, cons: UUID, prev: UUID) -> GraphDelta {
    let tail = editor.gid.get(entity: .uuid(cons), label: editor.schema.tailField)
    return GraphDelta.setting(entity: prev, label: editor.schema.tailField, value: tail)
}

func conses(_ editor: Editor, of entity: UUID) -> [(cons: UUID, head: Id)]? {
    var result: [(UUID, Id)] = []
    var current: Id = .uuid(entity)
    var visited: Set<UUID> = []
    while case .uuid(let uuid) = current {
        guard !visited.contains(uuid) else { return nil }
        visited.insert(uuid)
        guard let recordType = editor.gid.get(entity: current, label: editor.schema.recordField) else { return nil }
        if recordType == editor.schema.emptyRecord { return result }
        guard recordType == editor.schema.consRecord else { return nil }
        guard let head = editor.gid.get(entity: current, label: editor.schema.headField) else { return nil }
        result.append((uuid, head))
        guard let tail = editor.gid.get(entity: current, label: editor.schema.tailField) else { return nil }
        current = tail
    }
    return nil
}
