import AppKit

/// Top-level view bound to an Editor. Renders the editor's root value via the
/// projection machinery and forwards graph deltas to its content for
/// incremental update.
class RootView: FlippedView, Projection {
    var editor: Editor {
        didSet { rebuild() }
    }
    private var content: (any Projection)?

    init(editor: Editor) {
        self.editor = editor
        super.init(frame: .zero)
        rebuild()
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) {
        // For now, full rebuild on any delta. Will become incremental
        // once projections track reads and dispatch sharper updates.
        rebuild()
    }

    private func rebuild() {
        content?.removeFromSuperview()
        let ctx = ProjectionContext(
            entity: editor.root,
            gid: editor.gid,
            schema: editor.schema,
            editor: editor,
            ancestors: [],
            substitution: [:])
        let projection = createProjection(ctx)
        addSubview(projection)
        constrain(projection, toFill: self)
        content = projection
    }
}

/// Root dispatch: given a projection context, produce the right Projection
/// for the entity's value.
func createProjection(_ ctx: ProjectionContext) -> any Projection {
    guard let entity = ctx.entity else { return PlaceholderView(ctx) }

    if case .uuid(let uuid) = entity, ctx.ancestors.contains(uuid) {
        return CycleView(label: ctx.editor.name(of: entity) ?? "<cycle>")
    }

    let recordType = ctx.gid.get(entity: entity, label: ctx.schema.recordField)
    switch recordType {
    case ctx.schema.stringRecord?:
        if case .string(let s) = entity { return StringView(text: s) }
    case ctx.schema.numberRecord?:
        if case .number(let n) = entity { return NumberView(number: n) }
    case ctx.schema.consRecord?, ctx.schema.emptyRecord?:
        if case .uuid(let uuid) = entity { return ListView(ctx, entity: uuid) }
    default:
        break
    }

    if case .uuid(let uuid) = entity {
        return RecordView(ctx, entity: uuid)
    }

    let name = ctx.editor.name(of: entity) ?? "\(entity)"
    return TextView(text: name, style: .literal)
}
