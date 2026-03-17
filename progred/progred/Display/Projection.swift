import SwiftUI

typealias Render = (ProjectionContext) -> D?

struct ProjectionContext {
    let entity: Id
    let gid: any Gid
    let editor: Editor?
    let ancestors: Set<Id>
    private let schema: Schema

    init(entity: Id, gid: any Gid, schema: Schema, editor: Editor?, ancestors: Set<Id>) {
        self.entity = entity
        self.gid = gid
        self.schema = schema
        self.editor = editor
        self.ancestors = ancestors
    }

    var isCycle: Bool { ancestors.contains(entity) }
    var nameField: Id { schema.nameField }
    var recordField: Id { schema.recordField }
    var typeExpressionField: Id { schema.typeExpressionField }
    var typeParametersField: Id { schema.typeParametersField }
    var typeFunctionField: Id { schema.typeFunctionField }
    var fieldsField: Id { schema.fieldsField }
    var summandsField: Id { schema.summandsField }
    var headField: Id { schema.headField }
    var tailField: Id { schema.tailField }
    var typeParameterRecord: Id { schema.typeParameterRecord }
    var fieldRecord: Id { schema.fieldRecord }
    var recordRecord: Id { schema.recordRecord }
    var sumRecord: Id { schema.sumRecord }
    var applyRecord: Id { schema.applyRecord }
    var consRecord: Id { schema.consRecord }
    var emptyRecord: Id { schema.emptyRecord }

    func get(_ field: Id) -> Id? {
        gid.get(entity: entity, label: field)
    }

    func record() -> Id? {
        gid.get(entity: entity, label: recordField)
    }

    func name() -> String? {
        name(of: entity)
    }

    func name(of id: Id) -> String? {
        if case .string(let s) = gid.get(entity: id, label: nameField) { return s }
        return nil
    }

    func typeParams(of entity: Id) -> [Id]? {
        guard let listId = gid.get(entity: entity, label: typeParametersField) else { return nil }
        return listToArray(listId)
    }

    func listToArray(_ listNode: Id) -> [Id]? {
        var result: [Id] = []
        var current = listNode
        var seen: Set<Id> = []
        while seen.insert(current).inserted {
            let rec = gid.get(entity: current, label: recordField)
            if rec == emptyRecord { return result }
            guard rec == consRecord,
                  let head = gid.get(entity: current, label: headField),
                  let tail = gid.get(entity: current, label: tailField)
            else { return nil }
            result.append(head)
            current = tail
        }
        return nil
    }

    func descend(_ field: Id, render: Render? = nil) -> D {
        guard let value = get(field) else { return .placeholder }
        let actions = selectionActions(field: field)
        return .selectable(actions, child: descend(to: value, render: render))
    }

    func selectionActions(field: Id) -> SelectionActions {
        guard let editor, case .uuid(let uuid) = entity else { return SelectionActions() }
        return SelectionActions(onDelete: {
            editor.delete(entity: uuid, label: field)
        })
    }

    func descend(to entity: Id, render: Render? = nil) -> D {
        let childCtx = ProjectionContext(entity: entity, gid: gid, schema: schema, editor: editor, ancestors: ancestors.union([self.entity]))
        let d = render.flatMap { $0(childCtx) } ?? progred.project(childCtx)
        if childCtx.isCycle {
            return .collapse(defaultCollapsed: true, header: kernelHeader(ctx: childCtx), body: d)
        }
        return d
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
    case .string(let s): .text(s, .literal)
    case .number(let n): .text(String(n), .literal)
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
