import Foundation

func projectTypeParameter(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() == ctx.typeParameterRecord else { return nil }
    return .selectable(.line([.text("∀", .punctuation), ctx.descend(ctx.nameField)]))
}

func projectField(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() == ctx.fieldRecord else { return nil }

    return .selectable(.line([
        ctx.descend(ctx.nameField),
        .space,
        .text("→", .punctuation),
        .space,
        ctx.descend(ctx.typeExpressionField, projector: projectTypeExpression),
    ]))
}

func projectRecord(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() == ctx.recordRecord else { return nil }

    return .selectable(.collapse(collapsed: false,
        header: typeHeader(ctx: ctx),
        body: { labeled(ctx.fieldsField, ctx.descend(ctx.fieldsField), ctx: ctx) }))
}

func projectSum(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() == ctx.sumRecord else { return nil }

    return .selectable(.collapse(collapsed: false,
        header: typeHeader(ctx: ctx),
        body: { labeled(ctx.summandsField, ctx.descend(ctx.summandsField), ctx: ctx) }))
}

func projectApply(_ ctx: ProjectionContext) -> D? {
    guard ctx.record() == ctx.applyRecord else { return nil }
    guard let tfId = ctx.get(ctx.typeFunctionField),
          let typeParams = ctx.typeParams(of: tfId) else { return nil }

    let args = typeParams.map { tp in
        D.line([
            ctx.project(tp, projector: projectRef),
            .space,
            .text("→", .punctuation),
            .space,
            ctx.project(field: tp, projector: projectTypeExpression),
        ])
    }

    return .selectable(.line([
        ctx.project(tfId, projector: projectRef),
        inlineBrackets(open: "<", close: ">", args),
    ]))
}

let projectTypeExpression: Projector = { ctx in
    projectApply(ctx) ?? projectRef(ctx)
}

private func typeHeader(ctx: ProjectionContext) -> D {
    let keyword: D = ctx.record().flatMap { ctx.name(of: $0) }.map { .text($0, .keyword) } ?? .placeholder
    return .line([
        keyword,
        .space,
        ctx.descend(ctx.nameField),
        ctx.descend(ctx.typeParametersField,
            projector: projectList(open: "<", close: ">", inline: true)),
    ])
}
