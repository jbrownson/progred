import Foundation

typealias Render = (ProjectionContext) -> D?

struct ProjectionContext {
    let entity: UUID
    let schema: Schema
    let ancestors: Set<UUID>
    let label: String?

    var isCycle: Bool { ancestors.contains(entity) }

    func get(_ field: UUID) -> Id? {
        schema.gid.get(entity: .uuid(entity), label: .uuid(field))
    }

    func record() -> UUID? {
        schema.record(of: entity)
    }

    func name() -> String? {
        schema.name(of: entity)
    }

    func child(_ entity: UUID, label: String? = nil) -> ProjectionContext {
        ProjectionContext(entity: entity, schema: schema, ancestors: ancestors.union([self.entity]), label: label)
    }

    func descend(_ child: UUID, label: String? = nil) -> D {
        project(self.child(child, label: label))
    }
}

// MARK: - Dispatch

private let renders: [Render] = [
    // Domain layer
    // (future: projectRecord, projectSum, projectField, projectApply, ...)

    // Kernel layer
    projectKernel,
]

func project(_ ctx: ProjectionContext) -> D {
    if ctx.isCycle { return kernelHeader(ctx: ctx) }

    for render in renders {
        if let d = render(ctx) { return d }
    }

    return projectRaw(ctx)
}

// MARK: - Raw layer

private func projectRaw(_ ctx: ProjectionContext) -> D {
    let header: D = .identicon(ctx.entity)

    guard let raw = ctx.schema.gid.edges(entity: .uuid(ctx.entity)) else { return header }
    if raw.isEmpty { return header }

    let body: D = .block(raw.sorted { $0.key < $1.key }.map { label, value in
        rawEdge(label: label, value: value, ctx: ctx)
    })

    return .collapse(collapsed: false, label: header, body: body)
}

private func rawEdge(label: Id, value: Id, ctx: ProjectionContext) -> D {
    let labelD: D = switch label {
    case .uuid(let uuid): .identicon(uuid)
    case .string(let s): .text(s, .literal)
    case .number(let n): .text(String(n), .literal)
    }

    let valueD: D = switch value {
    case .uuid(let uuid): ctx.descend(uuid)
    case .string(let s): .text(s, .literal)
    case .number(let n): .text(String(n), .literal)
    }

    return .line([labelD, .text("→", .punctuation), valueD])
}
