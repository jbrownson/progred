import Foundation

typealias Render = (ProjectionContext) -> D?

struct ProjectionContext {
    let entity: Id
    let schema: Schema
    let ancestors: Set<Id>

    var isCycle: Bool { ancestors.contains(entity) }

    func get(_ field: Id) -> Id? {
        schema.gid.get(entity: entity, label: field)
    }

    func descend(_ field: Id, render: Render? = nil) -> D {
        guard let value = get(field) else { return .placeholder }
        return descend(to: value, render: render)
    }

    func descend(to entity: Id, render: Render? = nil) -> D {
        let childCtx = ProjectionContext(entity: entity, schema: schema, ancestors: ancestors.union([self.entity]))
        let d = render.flatMap { $0(childCtx) } ?? progred.project(childCtx)
        if childCtx.isCycle {
            return .collapse(defaultCollapsed: true, header: kernelHeader(ctx: childCtx), body: d)
        }
        return d
    }

    func project(_ id: Id, render: Render? = nil) -> D {
        let ctx = ProjectionContext(entity: id, schema: schema, ancestors: ancestors)
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
    if let name = ctx.schema.name(of: ctx.entity) { return .text(name, .literal) }
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

func labeled(_ field: Id, _ content: D, schema: Schema) -> D {
    let label: D = schema.name(of: field).map { .text($0, .label) } ?? .placeholder
    return .block([
        .line([label, .space, .text("→", .punctuation)]),
        .indent(content),
    ])
}
