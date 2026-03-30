import Foundation

typealias Render = (ProjectionContext) -> D?

struct ProjectionContext {
    let entity: Id?
    let path: Path
    let gid: any Gid
    let editor: Editor?
    let focus: Path?
    let ancestors: Set<Id>
    let readOnly: Bool
    private let schema: Schema

    init(entity: Id?, path: Path, gid: any Gid, schema: Schema, editor: Editor?, focus: Path?, ancestors: Set<Id>, readOnly: Bool = false) {
        self.entity = entity
        self.path = path
        self.gid = gid
        self.schema = schema
        self.editor = editor
        self.focus = focus
        self.ancestors = ancestors
        self.readOnly = readOnly
    }

    var isCycle: Bool {
        guard let entity else { return false }
        return ancestors.contains(entity)
    }
    var nameField: Id { schema.nameField }
    var recordField: Id { schema.recordField }
    var typeExpressionField: Id { schema.typeExpressionField }
    var typeParametersField: Id { schema.typeParametersField }
    var typeFunctionField: Id { schema.typeFunctionField }
    var fieldsField: Id { schema.fieldsField }
    var summandsField: Id { schema.summandsField }
    var headField: Id { schema.headField }
    var tailField: Id { schema.tailField }
    var insertField: Id { schema.insertField }
    var typeParameterRecord: Id { schema.typeParameterRecord }
    var fieldRecord: Id { schema.fieldRecord }
    var recordRecord: Id { schema.recordRecord }
    var sumRecord: Id { schema.sumRecord }
    var applyRecord: Id { schema.applyRecord }
    var consRecord: Id { schema.consRecord }
    var emptyRecord: Id { schema.emptyRecord }

    func get(_ field: Id) -> Id? {
        guard let entity else { return nil }
        return gid.get(entity: entity, label: field)
    }

    func record() -> Id? {
        get(recordField)
    }

    func name() -> String? {
        guard let entity else { return nil }
        return name(of: entity)
    }

    func name(of id: Id) -> String? {
        if case .string(let s) = gid.get(entity: id, label: nameField) { return s }
        return nil
    }

    func typeParams(of entity: Id) -> [Id]? {
        guard let listId = gid.get(entity: entity, label: typeParametersField) else { return nil }
        return listToArray(listId)?.map(\.head)
    }

    struct ListElement {
        let cons: Id
        let head: Id
    }

    func listToArray(_ listNode: Id) -> [ListElement]? {
        var result: [ListElement] = []
        var current = listNode
        var seen: Set<Id> = []
        while seen.insert(current).inserted {
            let rec = gid.get(entity: current, label: recordField)
            if rec == emptyRecord { return result }
            guard rec == consRecord,
                  let head = gid.get(entity: current, label: headField),
                  let tail = gid.get(entity: current, label: tailField)
            else { return nil }
            result.append(ListElement(cons: current, head: head))
            current = tail
        }
        return nil
    }

    func with(entity: Id?, path: Path, readOnly: Bool) -> ProjectionContext {
        ProjectionContext(entity: entity, path: path, gid: gid, schema: schema,
            editor: editor, focus: focus, ancestors: ancestors, readOnly: readOnly)
    }

    func descend(_ field: Id, render: Render? = nil) -> D {
        let childPath = path.child(field)
        let value = get(field)
        let childAncestors = entity.map { ancestors.union([$0]) } ?? ancestors
        let childInCycle = value.map { childAncestors.contains($0) } ?? false
        let childReadOnly = readOnly
            || (value.flatMap { gid.edges(entity: $0)?.readOnly } ?? false)
        let childCtx = ProjectionContext(
            entity: value, path: childPath, gid: gid, schema: schema,
            editor: editor, focus: focus,
            ancestors: childAncestors,
            readOnly: childReadOnly)
        let d = render.flatMap { $0(childCtx) } ?? progred.project(childCtx)
        let commit: Commit? = readOnly ? nil : { editor, id in
            editor.commit(path: childPath, value: id)
        }
        return .descend(Descend(
            path: childPath,
            readOnly: childReadOnly,
            inCycle: childInCycle,
            commit: commit,
            body: d))
    }

    func project(_ id: Id, render: Render? = nil) -> D {
        let ctx = ProjectionContext(entity: id, path: path, gid: gid, schema: schema, editor: editor, focus: focus, ancestors: ancestors)
        return render.flatMap({ $0(ctx) }) ?? progred.project(ctx)
    }

    func project(field: Id, render: Render? = nil) -> D {
        guard let value = get(field) else { return .placeholder }
        return project(value, render: render)
    }
}

// MARK: - Dispatch

private let renders: [Render] = [
    // MARK: Domain
    projectTypeParameter,
    projectField,
    projectApply,
    projectRecord,
    projectSum,

    // MARK: Kernel
    renderList(),
    projectKernel,
]

func project(_ ctx: ProjectionContext) -> D {
    for render in renders {
        if let d = render(ctx) { return d }
    }
    return projectRaw(ctx)
}

// MARK: - Shallow reference render

let renderRef: Render = { ctx in
    if let d = projectApply(ctx) { return d }
    if let name = ctx.name() { return .text(name, .literal) }
    return kernelHeader(ctx: ctx)
}

// MARK: - Raw header

func rawHeader(_ id: Id) -> D {
    switch id {
    case .uuid(let uuid): .identicon(uuid)
    case .string(let s): .stringEditor(s)
    case .number(let n): .numberEditor(n)
    }
}

// MARK: - Layout helpers

func labeled(_ field: Id, _ content: D, ctx: ProjectionContext) -> D {
    let label: D = ctx.name(of: field).map { .text($0, .label) } ?? .placeholder
    return .block([
        .line([label, .space, .text("→", .punctuation)]),
        .indent(content),
    ])
}
