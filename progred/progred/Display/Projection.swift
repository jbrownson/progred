import AppKit

/// A view that's the result of projecting a graph value. Knows how to apply
/// graph deltas to itself — either re-project or propagate to children
/// depending on what changed.
protocol Projection: NSView {
    func apply(_ delta: GraphDelta)
}

/// Inputs to a projection: where in the graph we are, what we have access to,
/// and the type-substitution context for resolving type-parameter applications.
struct ProjectionContext {
    let entity: Id?
    let gid: any Gid
    let schema: Schema
    let editor: Editor
    let ancestors: Set<UUID>
    let substitution: Substitution

    func descending(to value: Id?, throughEntity entity: UUID? = nil) -> ProjectionContext {
        let newAncestors = entity.map { ancestors.union([$0]) } ?? ancestors
        return ProjectionContext(
            entity: value,
            gid: gid,
            schema: schema,
            editor: editor,
            ancestors: newAncestors,
            substitution: substitution)
    }
}
