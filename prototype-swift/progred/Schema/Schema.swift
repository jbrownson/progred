import Foundation

struct Schema {
    var gid: ImmGid

    // MARK: - Fields
    let nameField: Id
    let recordField: Id
    let typeExpressionField: Id
    let typeParametersField: Id
    let typeFunctionField: Id
    let fieldsField: Id
    let summandsField: Id
    let headField: Id
    let tailField: Id
    let typesField: Id
    let insertField: Id

    // MARK: - Records
    let stringRecord: Id
    let numberRecord: Id
    let typeParameterRecord: Id
    let fieldRecord: Id
    let recordRecord: Id
    let sumRecord: Id
    let applyRecord: Id
    let consRecord: Id
    let emptyRecord: Id
    let libraryRecord: Id

    // MARK: - Sums
    let typeFunctionSum: Id
    let typeExpressionSum: Id
    let listSum: Id

    // MARK: - Type parameters
    let listT: Id

    // MARK: - Library instance
    let library: Id

    // MARK: - Graph queries

    func name(of entity: Id, gid: any Gid) -> String? {
        switch gid.get(entity: entity, label: nameField) {
        case .string(let s): s
        default: nil
        }
    }

    func record(of entity: Id, gid: any Gid) -> Id? {
        gid.get(entity: entity, label: recordField)
    }

    func listToArray(_ listNode: Id, gid: any Gid) -> [Id]? {
        var result: [Id] = []
        var current = listNode
        var seen: Set<Id> = []
        while seen.insert(current).inserted {
            let rec = gid.get(entity: current, label: recordField)
            if rec == emptyRecord {
                return result
            }
            guard rec == consRecord,
                  let head = gid.get(entity: current, label: headField),
                  let tail = gid.get(entity: current, label: tailField)
            else { return nil }
            result.append(head)
            current = tail
        }
        return nil
    }

    func fields(of entity: Id, gid: any Gid) -> [Id]? {
        guard let listId = gid.get(entity: entity, label: fieldsField) else { return nil }
        return listToArray(listId, gid: gid)
    }

    func typeParams(of entity: Id, gid: any Gid) -> [Id]? {
        guard let listId = gid.get(entity: entity, label: typeParametersField) else { return nil }
        return listToArray(listId, gid: gid)
    }

    func summands(of entity: Id, gid: any Gid) -> [Id]? {
        guard let listId = gid.get(entity: entity, label: summandsField) else { return nil }
        return listToArray(listId, gid: gid)
    }

    // MARK: - Bootstrap

    static func bootstrap() -> Schema {
        var gid = MutGid()

        // MARK: Generate all UUIDs─────────────────────────────

        let nameField = UUID()
        let recordField = UUID()
        let typeExpressionField = UUID()
        let typeParametersField = UUID()
        let typeFunctionField = UUID()
        let fieldsField = UUID()
        let summandsField = UUID()
        let headField = UUID()
        let tailField = UUID()
        let typesField = UUID()
        let insertField = UUID()

        let stringRecord = UUID()
        let numberRecord = UUID()
        let typeParameterRecord = UUID()
        let fieldRecord = UUID()
        let recordRecord = UUID()
        let sumRecord = UUID()
        let applyRecord = UUID()
        let consRecord = UUID()
        let emptyRecord = UUID()
        let libraryRecord = UUID()

        let typeFunctionSum = UUID()
        let typeExpressionSum = UUID()
        let listSum = UUID()

        let listT = UUID()

        let library = UUID()

        // MARK: Helpers────────────────────────────────────────

        func set(_ entity: UUID, _ label: UUID, _ value: UUID) {
            gid.set(entity: entity, label: .uuid(label), value: .uuid(value))
        }

        func setStr(_ entity: UUID, _ label: UUID, _ value: String) {
            gid.set(entity: entity, label: .uuid(label), value: .string(value))
        }

        func makeList(_ items: [UUID]) -> UUID {
            let empty = UUID()
            set(empty, recordField, emptyRecord)
            var current = empty
            for item in items.reversed() {
                let cons = UUID()
                set(cons, recordField, consRecord)
                set(cons, headField, item)
                set(cons, tailField, current)
                current = cons
            }
            return current
        }

        func makeApply(typeFunction tf: UUID, args: [(param: UUID, arg: UUID)]) -> UUID {
            let apply = UUID()
            set(apply, recordField, applyRecord)
            set(apply, typeFunctionField, tf)
            for (param, arg) in args {
                set(apply, param, arg)
            }
            return apply
        }

        // MARK: Apply nodes for generic type references────────

        let listOfTypeParam = makeApply(typeFunction: listSum, args: [(listT, typeParameterRecord)])
        let listOfField = makeApply(typeFunction: listSum, args: [(listT, fieldRecord)])
        let listOfTypeExpr = makeApply(typeFunction: listSum, args: [(listT, typeExpressionSum)])
        let listOfT = makeApply(typeFunction: listSum, args: [(listT, listT)])
        let listOfTypeFunction = makeApply(typeFunction: listSum, args: [(listT, typeFunctionSum)])

        // MARK: Field declarations─────────────────────────────

        func declareField(_ uuid: UUID, name: String, typeExpr: UUID) {
            set(uuid, recordField, fieldRecord)
            setStr(uuid, nameField, name)
            set(uuid, typeExpressionField, typeExpr)
        }

        declareField(nameField, name: "name", typeExpr: stringRecord)
        declareField(recordField, name: "record", typeExpr: recordRecord)
        declareField(typeExpressionField, name: "type expression", typeExpr: typeExpressionSum)
        declareField(typeParametersField, name: "type parameters", typeExpr: listOfTypeParam)
        declareField(typeFunctionField, name: "type function", typeExpr: typeFunctionSum)
        declareField(fieldsField, name: "fields", typeExpr: listOfField)
        declareField(summandsField, name: "summands", typeExpr: listOfTypeExpr)
        declareField(headField, name: "head", typeExpr: listT)
        declareField(tailField, name: "tail", typeExpr: listOfT)
        declareField(typesField, name: "types", typeExpr: listOfTypeFunction)
        declareField(insertField, name: "insert", typeExpr: listT)

        // MARK: Record declarations────────────────────────────

        func declareRecord(_ uuid: UUID, name: String, typeParams: [UUID], fields: [UUID]) {
            set(uuid, recordField, recordRecord)
            setStr(uuid, nameField, name)
            set(uuid, typeParametersField, makeList(typeParams))
            set(uuid, fieldsField, makeList(fields))
        }

        declareRecord(stringRecord, name: "String", typeParams: [], fields: [])
        declareRecord(numberRecord, name: "Number", typeParams: [], fields: [])
        declareRecord(typeParameterRecord, name: "Type Parameter", typeParams: [], fields: [nameField])
        declareRecord(fieldRecord, name: "Field", typeParams: [], fields: [nameField, typeExpressionField])
        declareRecord(recordRecord, name: "Record", typeParams: [], fields: [nameField, typeParametersField, fieldsField])
        declareRecord(sumRecord, name: "Sum", typeParams: [], fields: [nameField, typeParametersField, summandsField])
        declareRecord(applyRecord, name: "Apply", typeParams: [], fields: [typeFunctionField])
        declareRecord(consRecord, name: "Cons", typeParams: [], fields: [headField, tailField])
        declareRecord(emptyRecord, name: "Empty", typeParams: [], fields: [])
        declareRecord(libraryRecord, name: "Library", typeParams: [], fields: [nameField, typesField, fieldsField])

        // MARK: Sum declarations───────────────────────────────

        func declareSum(_ uuid: UUID, name: String, typeParams: [UUID], summands: [UUID]) {
            set(uuid, recordField, sumRecord)
            setStr(uuid, nameField, name)
            set(uuid, typeParametersField, makeList(typeParams))
            set(uuid, summandsField, makeList(summands))
        }

        declareSum(typeFunctionSum, name: "Type Function", typeParams: [], summands: [recordRecord, sumRecord])
        declareSum(typeExpressionSum, name: "Type Expression", typeParams: [], summands: [recordRecord, sumRecord, applyRecord, typeParameterRecord])
        declareSum(listSum, name: "List", typeParams: [listT], summands: [consRecord, emptyRecord])

        // MARK: Type parameters────────────────────────────────

        set(listT, recordField, typeParameterRecord)
        setStr(listT, nameField, "T")

        // MARK: Library instance───────────────────────────────

        set(library, recordField, libraryRecord)
        setStr(library, nameField, "Core")
        set(library, typesField, makeList([
            stringRecord, numberRecord, typeParameterRecord,
            fieldRecord, recordRecord, sumRecord, applyRecord,
            consRecord, emptyRecord, libraryRecord,
            typeFunctionSum, typeExpressionSum, listSum,
        ]))
        set(library, fieldsField, makeList([
            nameField, recordField, typeExpressionField,
            typeParametersField, typeFunctionField, typesField,
        ]))

        // MARK: Assemble───────────────────────────────────────

        func id(_ uuid: UUID) -> Id { .uuid(uuid) }

        return Schema(
            gid: gid.frozen(),
            nameField: id(nameField),
            recordField: id(recordField),
            typeExpressionField: id(typeExpressionField),
            typeParametersField: id(typeParametersField),
            typeFunctionField: id(typeFunctionField),
            fieldsField: id(fieldsField),
            summandsField: id(summandsField),
            headField: id(headField),
            tailField: id(tailField),
            typesField: id(typesField),
            insertField: id(insertField),
            stringRecord: id(stringRecord),
            numberRecord: id(numberRecord),
            typeParameterRecord: id(typeParameterRecord),
            fieldRecord: id(fieldRecord),
            recordRecord: id(recordRecord),
            sumRecord: id(sumRecord),
            applyRecord: id(applyRecord),
            consRecord: id(consRecord),
            emptyRecord: id(emptyRecord),
            libraryRecord: id(libraryRecord),
            typeFunctionSum: id(typeFunctionSum),
            typeExpressionSum: id(typeExpressionSum),
            listSum: id(listSum),
            listT: id(listT),
            library: id(library)
        )
    }
}
