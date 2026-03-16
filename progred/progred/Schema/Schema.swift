import Foundation

struct Schema {
    var gid: MutGid

    // Field declarations (their UUIDs serve as edge labels)
    let nameField: UUID
    let recordField: UUID
    let typeExpressionField: UUID
    let typeParametersField: UUID
    let typeFunctionField: UUID
    let fieldsField: UUID
    let summandsField: UUID
    let headField: UUID
    let tailField: UUID
    let typesField: UUID

    // Record declarations
    let stringRecord: UUID
    let numberRecord: UUID
    let typeParameterRecord: UUID
    let fieldRecord: UUID
    let recordRecord: UUID
    let sumRecord: UUID
    let applyRecord: UUID
    let consRecord: UUID
    let emptyRecord: UUID
    let libraryRecord: UUID

    // Sum declarations
    let typeFunctionSum: UUID
    let typeExpressionSum: UUID
    let listSum: UUID

    // Type parameters
    let listT: UUID

    // Library instance
    let library: UUID

    // MARK: - Graph queries

    func name(of entity: UUID) -> String? {
        switch gid.get(entity: .uuid(entity), label: .uuid(nameField)) {
        case .string(let s): s
        default: nil
        }
    }

    func record(of entity: UUID) -> UUID? {
        gid.get(entity: .uuid(entity), label: .uuid(recordField))?.asUUID
    }

    func listToArray(_ listNode: UUID) -> [UUID] {
        var result: [UUID] = []
        var current = listNode
        var seen: Set<UUID> = []
        while seen.insert(current).inserted {
            if record(of: current) == consRecord {
                if let head = gid.get(entity: .uuid(current), label: .uuid(headField))?.asUUID {
                    result.append(head)
                }
                if let tail = gid.get(entity: .uuid(current), label: .uuid(tailField))?.asUUID {
                    current = tail
                } else {
                    break
                }
            } else {
                break
            }
        }
        return result
    }

    func fields(of rec: UUID) -> [UUID] {
        guard let listId = gid.get(entity: .uuid(rec), label: .uuid(fieldsField))?.asUUID else {
            return []
        }
        return listToArray(listId)
    }

    func typeParams(of decl: UUID) -> [UUID] {
        guard let listId = gid.get(entity: .uuid(decl), label: .uuid(typeParametersField))?.asUUID else {
            return []
        }
        return listToArray(listId)
    }

    func summands(of sum: UUID) -> [UUID] {
        guard let listId = gid.get(entity: .uuid(sum), label: .uuid(summandsField))?.asUUID else {
            return []
        }
        return listToArray(listId)
    }

    // MARK: - Bootstrap

    static func bootstrap() -> Schema {
        var gid = MutGid()

        // ── Generate all UUIDs ──────────────────────────────

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

        // ── Helpers ─────────────────────────────────────────

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

        // ── Apply nodes for generic type references ─────────

        let listOfTypeParam = makeApply(typeFunction: listSum, args: [(listT, typeParameterRecord)])
        let listOfField = makeApply(typeFunction: listSum, args: [(listT, fieldRecord)])
        let listOfTypeExpr = makeApply(typeFunction: listSum, args: [(listT, typeExpressionSum)])
        let listOfT = makeApply(typeFunction: listSum, args: [(listT, listT)])
        let listOfTypeFunction = makeApply(typeFunction: listSum, args: [(listT, typeFunctionSum)])

        // ── Field declarations ──────────────────────────────

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

        // ── Record declarations ─────────────────────────────

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

        // ── Sum declarations ────────────────────────────────

        func declareSum(_ uuid: UUID, name: String, typeParams: [UUID], summands: [UUID]) {
            set(uuid, recordField, sumRecord)
            setStr(uuid, nameField, name)
            set(uuid, typeParametersField, makeList(typeParams))
            set(uuid, summandsField, makeList(summands))
        }

        declareSum(typeFunctionSum, name: "Type Function", typeParams: [], summands: [recordRecord, sumRecord])
        declareSum(typeExpressionSum, name: "Type Expression", typeParams: [], summands: [recordRecord, sumRecord, applyRecord, typeParameterRecord])
        declareSum(listSum, name: "List", typeParams: [listT], summands: [consRecord, emptyRecord])

        // ── Type parameters ─────────────────────────────────

        set(listT, recordField, typeParameterRecord)
        setStr(listT, nameField, "T")

        // ── Library instance ────────────────────────────────

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

        // ── Assemble ────────────────────────────────────────

        return Schema(
            gid: gid,
            nameField: nameField,
            recordField: recordField,
            typeExpressionField: typeExpressionField,
            typeParametersField: typeParametersField,
            typeFunctionField: typeFunctionField,
            fieldsField: fieldsField,
            summandsField: summandsField,
            headField: headField,
            tailField: tailField,
            typesField: typesField,
            stringRecord: stringRecord,
            numberRecord: numberRecord,
            typeParameterRecord: typeParameterRecord,
            fieldRecord: fieldRecord,
            recordRecord: recordRecord,
            sumRecord: sumRecord,
            applyRecord: applyRecord,
            consRecord: consRecord,
            emptyRecord: emptyRecord,
            libraryRecord: libraryRecord,
            typeFunctionSum: typeFunctionSum,
            typeExpressionSum: typeExpressionSum,
            listSum: listSum,
            listT: listT,
            library: library
        )
    }
}
