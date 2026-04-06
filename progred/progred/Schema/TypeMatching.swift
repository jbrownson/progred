import Foundation

typealias Substitution = [Id: Id]

func matches(_ value: Id, _ type: Id, _ substitution: Substitution,
             gid: any Gid, schema: Schema) -> Bool? {
    var visited = Set<IdPair>()
    return matchesImpl(value, type, substitution, gid: gid, schema: schema, visited: &visited)
}

func admits(_ record: Id, _ type: Id, _ substitution: Substitution,
            gid: any Gid, schema: Schema) -> Bool? {
    admitsImpl(record, type, substitution, gid: gid, schema: schema)
}

// MARK: -

private struct IdPair: Hashable {
    let value: Id, type: Id
}

private func resolveParam(_ type: Id, _ substitution: Substitution,
                           gid: any Gid, schema: Schema) -> Id? {
    var current = type
    var seen = Set<Id>()
    while gid.get(entity: current, label: schema.recordField) == schema.typeParameterRecord {
        guard seen.insert(current).inserted, let resolved = substitution[current] else { return nil }
        current = resolved
    }
    return current
}

private func admitsImpl(_ record: Id, _ type: Id, _ substitution: Substitution,
                         gid: any Gid, schema: Schema) -> Bool? {
    switch gid.get(entity: type, label: schema.recordField) {
    case schema.typeParameterRecord:
        guard let resolved = resolveParam(type, substitution, gid: gid, schema: schema)
        else { return nil }
        return admitsImpl(record, resolved, substitution, gid: gid, schema: schema)

    case schema.applyRecord:
        guard let tf = gid.get(entity: type, label: schema.typeFunctionField),
              let extended = bindTypeArgs(type, tf, substitution, gid: gid, schema: schema)
        else { return nil }
        return admitsImpl(record, tf, extended, gid: gid, schema: schema)

    case schema.sumRecord:
        guard let sums = schema.summands(of: type, gid: gid) else { return nil }
        var sawNil = false
        for summand in sums {
            switch admitsImpl(record, summand, substitution, gid: gid, schema: schema) {
            case true: return true
            case nil: sawNil = true
            case false: break
            }
        }
        return sawNil ? nil : false

    case schema.recordRecord:
        return record == type

    default:
        return nil
    }
}

private func matchesImpl(_ value: Id, _ type: Id, _ substitution: Substitution,
                         gid: any Gid, schema: Schema, visited: inout Set<IdPair>) -> Bool? {
    switch gid.get(entity: type, label: schema.recordField) {
    case schema.typeParameterRecord:
        guard let resolved = resolveParam(type, substitution, gid: gid, schema: schema)
        else { return nil }
        return matchesImpl(value, resolved, substitution, gid: gid, schema: schema, visited: &visited)

    case schema.applyRecord:
        guard let tf = gid.get(entity: type, label: schema.typeFunctionField),
              let extended = bindTypeArgs(type, tf, substitution, gid: gid, schema: schema)
        else { return nil }
        return matchesImpl(value, tf, extended, gid: gid, schema: schema, visited: &visited)

    case schema.sumRecord:
        guard let sums = schema.summands(of: type, gid: gid) else { return nil }
        var sawNil = false
        for summand in sums {
            switch matchesImpl(value, summand, substitution, gid: gid, schema: schema, visited: &visited) {
            case true: return true
            case nil: sawNil = true
            case false: break
            }
        }
        return sawNil ? nil : false

    case schema.recordRecord:
        guard gid.get(entity: value, label: schema.recordField) == type else { return false }
        guard visited.insert(IdPair(value: value, type: type)).inserted else { return true }
        guard let fs = schema.fields(of: type, gid: gid) else { return nil }
        for field in fs {
            guard gid.get(entity: field, label: schema.recordField) == schema.fieldRecord,
                  let typeExpr = gid.get(entity: field, label: schema.typeExpressionField)
            else { return nil }
            guard let fieldValue = gid.get(entity: value, label: field)
            else { return false }
            guard let result = matchesImpl(fieldValue, typeExpr, substitution, gid: gid, schema: schema, visited: &visited)
            else { return nil }
            guard result else { return false }
        }
        return true

    default:
        return nil
    }
}

func bindTypeArgs(_ apply: Id, _ tf: Id, _ base: Substitution,
                           gid: any Gid, schema: Schema) -> Substitution? {
    guard let params = schema.typeParams(of: tf, gid: gid) else { return nil }
    var extended = base
    for param in params {
        guard let arg = gid.get(entity: apply, label: param) else { return nil }
        extended[param] = resolveArg(arg, base, gid: gid, schema: schema)
    }
    return extended
}

func resolveArg(_ arg: Id, _ substitution: Substitution,
                          gid: any Gid, schema: Schema) -> Id {
    if gid.get(entity: arg, label: schema.recordField) == schema.typeParameterRecord,
       let resolved = substitution[arg] {
        return resolved
    }
    return arg
}
