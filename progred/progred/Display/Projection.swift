import Foundation

typealias Render = (ProjectionContext) -> D?

struct ProjectionContext {
    let entity: Id?
    let gid: any Gid
    let editor: Editor?
    let ancestors: Set<Id>
    let commit: Commit?
    let substitution: Substitution
    private let schema: Schema

    init(entity: Id?, gid: any Gid, schema: Schema, editor: Editor?, ancestors: Set<Id>,
         commit: Commit? = nil, substitution: Substitution = [:]) {
        self.entity = entity
        self.gid = gid
        self.schema = schema
        self.editor = editor
        self.ancestors = ancestors
        self.commit = commit
        self.substitution = substitution
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
        return conses(listId)?.cells.compactMap { gid.get(entity: $0, label: headField) }
    }

    func conses(_ listNode: Id) -> (cells: [Id], empty: Id, readOnly: Bool)? {
        var result: [Id] = []
        var readOnly = false
        var current = listNode
        var seen: Set<Id> = []
        while seen.insert(current).inserted {
            guard let edges = gid.edges(entity: current) else { return nil }
            if edges[recordField] == emptyRecord { return (result, current, readOnly) }
            guard edges[recordField] == consRecord,
                  let tail = edges[tailField]
            else { return nil }
            if edges.readOnly { readOnly = true }
            result.append(current)
            current = tail
        }
        return nil
    }

    func resolveExpectedType(for field: Id) -> Id? {
        guard let typeExpr = gid.get(entity: field, label: typeExpressionField) else { return nil }
        if gid.get(entity: typeExpr, label: recordField) == typeParameterRecord {
            return substitution[typeExpr]
        }
        return typeExpr
    }

    func with(entity: Id?, ancestors: Set<Id>? = nil, commit: Commit?) -> ProjectionContext {
        ProjectionContext(entity: entity, gid: gid, schema: schema,
            editor: editor, ancestors: ancestors ?? self.ancestors,
            commit: commit, substitution: substitution)
    }

    func descend(_ field: Id, render: Render? = nil, commit: Commit? = nil) -> D {
        let value = get(field)
        let childAncestors = entity.map { ancestors.union([$0]) } ?? ancestors
        let childInCycle = value.map { childAncestors.contains($0) } ?? false
        let edgeReadOnly = value.flatMap { gid.edges(entity: $0)?.readOnly } ?? false
        let edgeCommit: Commit? = self.commit == nil
            ? nil
            : commit ?? entity.flatMap { parent in
                guard case .uuid(let uuid) = parent else { return nil }
                return { editor, id in editor.commit(entity: uuid, label: field, value: id) }
            }
        let expectedType = resolveExpectedType(for: field)
        let typeExpr = gid.get(entity: field, label: typeExpressionField)
        let childSubstitution: Substitution = {
            guard let typeExpr,
                  gid.get(entity: typeExpr, label: recordField) == applyRecord,
                  let tf = gid.get(entity: typeExpr, label: typeFunctionField),
                  let extended = bindTypeArgs(typeExpr, tf, substitution, gid: gid, schema: schema)
            else { return substitution }
            return extended
        }()
        let childCtx = ProjectionContext(
            entity: value, gid: gid, schema: schema,
            editor: editor, ancestors: childAncestors,
            commit: edgeReadOnly ? nil : edgeCommit,
            substitution: childSubstitution)
        let d = render.flatMap { $0(childCtx) } ?? progred.project(childCtx)
        return .descend(Descend(
            inCycle: childInCycle,
            commit: edgeCommit,
            expectedType: expectedType,
            substitution: substitution,
            body: d))
    }

    func project(_ id: Id, render: Render? = nil) -> D {
        let ctx = ProjectionContext(entity: id, gid: gid, schema: schema, editor: editor, ancestors: ancestors)
        return render.flatMap({ $0(ctx) }) ?? progred.project(ctx)
    }

    func project(field: Id, render: Render? = nil) -> D {
        guard let value = get(field) else { return .placeholder }
        return project(value, render: render)
    }
}

// MARK: - Dispatch

private let projectPrimitive: Render = { ctx in
    guard let entity = ctx.entity else { return nil }
    switch entity {
    case .string, .number: return rawHeader(entity)
    case .uuid: return nil
    }
}

private let renders: [Render] = [
    // MARK: Domain
    projectTypeParameter,
    projectField,
    projectApply,
    projectRecord,
    projectSum,

    // MARK: Kernel
    renderList(),
    projectPrimitive,
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
