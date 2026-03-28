import Foundation

func projectRaw(_ ctx: ProjectionContext) -> D {
    guard let entity = ctx.entity else { return .placeholder }
    let header = rawHeader(entity)

    guard let raw = ctx.gid.edges(entity: entity) else { return header }
    if raw.data.isEmpty { return header }

    return .collapse(collapsed: false, header: header) {
        .block(raw.data.sorted { $0.key < $1.key }.map { label, _ in
            labeled(label, ctx.descend(label), ctx: ctx)
        })
    }
}
