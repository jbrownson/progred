import Foundation

func projectTypeParameter(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) == ctx.schema.typeParameterRecord else { return nil }
    return .line([.text("∀", .punctuation), ctx.descend(ctx.schema.nameField)])
}

func projectField(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) == ctx.schema.fieldRecord else { return nil }

    return .line([
        ctx.descend(ctx.schema.nameField),
        .space,
        .text("→", .punctuation),
        .space,
        ctx.project(field: ctx.schema.typeExpressionField, render: renderRef),
    ])
}

func projectRecord(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) == ctx.schema.recordRecord else { return nil }

    return .collapse(
        header: typeHeader(recordType: ctx.schema.recordRecord, ctx: ctx),
        body: labeled(ctx.schema.fieldsField, ctx.descend(ctx.schema.fieldsField), schema: ctx.schema))
}

func projectSum(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) == ctx.schema.sumRecord else { return nil }

    return .collapse(
        header: typeHeader(recordType: ctx.schema.sumRecord, ctx: ctx),
        body: labeled(ctx.schema.summandsField, ctx.descend(ctx.schema.summandsField), schema: ctx.schema))
}

func projectApply(_ ctx: ProjectionContext) -> D? {
    guard ctx.schema.record(of: ctx.entity) == ctx.schema.applyRecord else { return nil }
    guard let tfId = ctx.get(ctx.schema.typeFunctionField),
          let typeParams = ctx.schema.typeParams(of: tfId) else { return nil }

    let args = typeParams.map { tp in
        D.line([
            ctx.project(tp, render: renderRef),
            .space,
            .text("→", .punctuation),
            .space,
            ctx.project(field: tp, render: renderRef),
        ])
    }

    return .line([
        ctx.project(tfId, render: renderRef),
        inlineBrackets(open: "<", close: ">", args),
    ])
}

private func typeHeader(recordType: Id, ctx: ProjectionContext) -> D {
    let keyword: D = ctx.schema.name(of: recordType).map { .text($0, .keyword) } ?? .placeholder
    return .line([
        keyword,
        .space,
        ctx.descend(ctx.schema.nameField),
        ctx.project(field: ctx.schema.typeParametersField,
            render: renderList(open: "<", close: ">", inline: true, elementRender: renderRef)),
    ])
}
