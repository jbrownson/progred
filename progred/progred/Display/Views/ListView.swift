import AppKit

/// Renders a cons chain — a graph value whose record edge points to consRecord
/// or emptyRecord. Walks tail edges, projecting each head element.
class ListView: FlippedView, Projection {
    private let ctx: ProjectionContext
    private let entity: UUID
    private var stack: NSStackView!

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
        stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        constrain(stack, toFill: self)

        let elements = walkCons(from: entity, gid: ctx.gid, schema: ctx.schema)
        for element in elements {
            let elementCtx = ctx.descending(to: element, throughEntity: entity)
            let view = createProjection(elementCtx)
            stack.addArrangedSubview(view)
        }
    }
}

/// Walk a cons chain rooted at `entity`, collecting the head value at each cons.
/// Stops at emptyRecord or any other non-cons entity (graceful for malformed chains).
func walkCons(from entity: UUID, gid: any Gid, schema: Schema) -> [Id] {
    var result: [Id] = []
    var current: Id = .uuid(entity)
    var visited: Set<UUID> = []
    while case .uuid(let uuid) = current {
        guard !visited.contains(uuid) else { break }  // cycle guard
        visited.insert(uuid)
        guard let recordType = gid.get(entity: current, label: schema.recordField) else { break }
        if recordType == schema.emptyRecord { break }
        guard recordType == schema.consRecord else { break }
        guard let head = gid.get(entity: current, label: schema.headField) else { break }
        result.append(head)
        guard let tail = gid.get(entity: current, label: schema.tailField) else { break }
        current = tail
    }
    return result
}
