import Foundation

func projectRaw(_ ctx: ProjectionContext) -> D {
    let header = rawHeader(ctx.entity)

    guard let raw = ctx.gid.edges(entity: ctx.entity) else { return header }
    if raw.data.isEmpty { return header }

    let body: D = .block(raw.data.sorted { $0.key < $1.key }.map { label, value in
        rawEdge(label: label, value: value, ctx: ctx)
    })

    return .collapse(header: header, body: body)
}

private func rawEdge(label: Id, value: Id, ctx: ProjectionContext) -> D {
    let labelD: D = switch label {
    case .uuid(let uuid): .identicon(uuid)
    case .string(let s): .text(s, .literal)
    case .number(let n): .text(String(n), .literal)
    }

    let valueD: D = switch value {
    case .uuid: ctx.descend(to: value).d
    case .string(let s): .text(s, .literal)
    case .number(let n): .text(String(n), .literal)
    }

    return .line([labelD, .space, .text("→", .punctuation), .space, valueD])
}
