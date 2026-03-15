import Foundation

func projectHeader(entity: UUID, schema: Schema, label: String? = nil) -> D {
    var parts: [D] = []
    if let label {
        parts.append(.text(label, .label))
        parts.append(.text(": ", .punctuation))
    }
    if let recName = schema.record(of: entity).flatMap({ schema.name(of: $0) }) {
        parts.append(.text(recName, .typeRef))
    }
    if let entityName = schema.name(of: entity) {
        parts.append(.text(entityName, .literal))
    }
    if schema.record(of: entity) == nil && schema.name(of: entity) == nil {
        parts.append(.identicon(entity))
    }
    return parts.count == 1 ? parts[0] : .line(parts)
}

func project(entity: UUID, schema: Schema, ancestors: Set<UUID> = [], label: String? = nil) -> D {
    let header = projectHeader(entity: entity, schema: schema, label: label)

    if ancestors.contains(entity) {
        return header
    }

    guard let raw = schema.gid.edges(entity: .uuid(entity)) else { return header }
    let edges = raw
        .filter { $0.key != .uuid(schema.nameField) && $0.key != .uuid(schema.recordField) }
        .sorted { $0.key < $1.key }

    if edges.isEmpty { return header }

    let childAncestors = ancestors.union([entity])
    let body: D = .block(edges.map { label, value in
        projectEdge(label: label, value: value, schema: schema, ancestors: childAncestors)
    })

    return .collapse(collapsed: false, label: header, body: body)
}

private func projectEdge(label: Id, value: Id, schema: Schema, ancestors: Set<UUID>) -> D {
    let labelName = label.asUUID.flatMap { schema.name(of: $0) } ?? "\(label)"

    switch value {
    case .string(let s):
        return .line([.text(labelName, .label), .text(": ", .punctuation), .text(s, .literal)])
    case .number(let n):
        return .line([.text(labelName, .label), .text(": ", .punctuation), .text(String(n), .literal)])
    case .uuid(let uuid):
        if isList(uuid, schema: schema) {
            let elements = schema.listToArray(uuid)
            let elementDs = elements.map { D.entity($0, label: nil, ancestors: ancestors) }
            return .line([
                .text(labelName, .label),
                .text(": ", .punctuation),
                .bracketed(open: "[", close: "]",
                    body: .list(separator: ",", elements: elementDs)),
            ])
        } else {
            return .descend(label: label, child:
                .entity(uuid, label: labelName, ancestors: ancestors))
        }
    }
}

private func isList(_ uuid: UUID, schema: Schema) -> Bool {
    guard let rec = schema.record(of: uuid) else { return false }
    return rec == schema.consRecord || rec == schema.emptyRecord
}
