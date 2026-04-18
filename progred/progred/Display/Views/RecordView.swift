import AppKit

/// Generic record rendering: header (record-type name) and a vertical stack
/// of "fieldName: value" rows, with one row per field declared by the
/// record's type.
class RecordView: FlippedView, Projection {
    private let ctx: ProjectionContext
    private let entity: UUID

    init(_ ctx: ProjectionContext, entity: UUID) {
        self.ctx = ctx
        self.entity = entity
        super.init(frame: .zero)
        rebuild()
    }

    required init?(coder: NSCoder) { fatalError() }

    func apply(_ delta: GraphDelta) { rebuild() }

    private func rebuild() {
        subviews.forEach { $0.removeFromSuperview() }

        let recordType = ctx.gid.get(entity: .uuid(entity), label: ctx.schema.recordField)
        let typeName = recordType.flatMap { ctx.editor.name(of: $0) } ?? "?"

        let header = styledLabel(typeName, .typeRef)

        var rows: [NSView] = [header]
        if let recordType,
           case .uuid(let typeUuid) = recordType,
           let fieldsListId = ctx.gid.get(entity: recordType, label: ctx.schema.fieldsField),
           case .uuid(let fieldsUuid) = fieldsListId {
            let fields = walkCons(from: fieldsUuid, gid: ctx.gid, schema: ctx.schema)
            for field in fields {
                rows.append(makeFieldRow(field: field))
            }
            _ = typeUuid  // (suppresses unused warning if record type itself isn't followed)
        }

        let stack = NSStackView(views: rows)
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        constrain(stack, toFill: self)
    }

    private func makeFieldRow(field: Id) -> NSView {
        let fieldName = ctx.editor.name(of: field) ?? "?"
        let value = ctx.gid.get(entity: .uuid(entity), label: field)
        let valueCtx = ctx.descending(to: value, throughEntity: entity)
        let valueView = createProjection(valueCtx)

        let row = NSStackView(views: [
            styledLabel("\(fieldName):", .label),
            valueView,
        ])
        row.spacing = 4
        row.orientation = .horizontal
        row.alignment = .firstBaseline
        return row
    }
}
