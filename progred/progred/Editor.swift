import Foundation
import HashTreeCollections

class Editor {
    let schema: Schema
    var document: MutGid
    var root: Id?
    let onChange: (GraphDelta) -> Void

    init(schema: Schema, document: MutGid = MutGid(), root: Id? = nil, onChange: @escaping (GraphDelta) -> Void) {
        self.schema = schema
        self.document = document
        self.root = root
        self.onChange = onChange
    }

    var gid: StackedGid<StackedGid<MutGid, ImmGid>, PrimitiveGid> {
        StackedGid(
            top: StackedGid(top: document, bottom: schema.gid),
            bottom: PrimitiveGid(
                recordField: schema.recordField,
                stringRecord: schema.stringRecord,
                numberRecord: schema.numberRecord))
    }

    func name(of entity: Id) -> String? {
        gid.get(entity: entity, label: schema.nameField)?.asString
    }

    func apply(_ delta: GraphDelta) {
        guard !delta.isEmpty else { return }
        document.apply(delta)
        onChange(delta)
    }

    func setRoot(_ id: Id?) {
        root = id
        // Root change isn't expressible as a graph delta (it's editor state,
        // not a graph edge). Notify with empty delta so consumers know
        // something changed and can re-read root.
        onChange(.empty)
    }

    static func withSampleDocument(onChange: @escaping (GraphDelta) -> Void) -> Editor {
        let schema = Schema.bootstrap()
        var document = MutGid()

        func set(_ entity: UUID, _ label: Id, _ value: Id) {
            document.commit(entity: entity, label: label, value: value)
        }

        func makeList(_ items: [Id]) -> UUID {
            let empty = UUID()
            set(empty, schema.recordField, schema.emptyRecord)
            var current: Id = .uuid(empty)
            for item in items.reversed() {
                let cons = UUID()
                set(cons, schema.recordField, schema.consRecord)
                set(cons, schema.headField, item)
                set(cons, schema.tailField, current)
                current = .uuid(cons)
            }
            return current.asUUID!
        }

        // Option<T> = Some | None
        let optionT = UUID()
        set(optionT, schema.recordField, schema.typeParameterRecord)
        set(optionT, schema.nameField, .string("T"))

        let valueField = UUID()
        set(valueField, schema.recordField, schema.fieldRecord)
        set(valueField, schema.nameField, .string("value"))
        set(valueField, schema.typeExpressionField, .uuid(optionT))

        let someRecord = UUID()
        set(someRecord, schema.recordField, schema.recordRecord)
        set(someRecord, schema.nameField, .string("Some"))
        set(someRecord, schema.fieldsField, .uuid(makeList([.uuid(valueField)])))
        set(someRecord, schema.typeParametersField, .uuid(makeList([])))

        let noneRecord = UUID()
        set(noneRecord, schema.recordField, schema.recordRecord)
        set(noneRecord, schema.nameField, .string("None"))
        set(noneRecord, schema.fieldsField, .uuid(makeList([])))
        set(noneRecord, schema.typeParametersField, .uuid(makeList([])))

        let optionSum = UUID()
        set(optionSum, schema.recordField, schema.sumRecord)
        set(optionSum, schema.nameField, .string("Option"))
        set(optionSum, schema.typeParametersField, .uuid(makeList([.uuid(optionT)])))
        set(optionSum, schema.summandsField, .uuid(makeList([.uuid(someRecord), .uuid(noneRecord)])))

        // Person { name: String, age: Option<Number> }
        let ageField = UUID()
        set(ageField, schema.recordField, schema.fieldRecord)
        set(ageField, schema.nameField, .string("age"))
        let optionOfNumber = UUID()
        set(optionOfNumber, schema.recordField, schema.applyRecord)
        set(optionOfNumber, schema.typeFunctionField, .uuid(optionSum))
        set(optionOfNumber, .uuid(optionT), schema.numberRecord)
        set(ageField, schema.typeExpressionField, .uuid(optionOfNumber))

        let personRecord = UUID()
        set(personRecord, schema.recordField, schema.recordRecord)
        set(personRecord, schema.nameField, .string("Person"))
        set(personRecord, schema.fieldsField, .uuid(makeList([schema.nameField, .uuid(ageField)])))
        set(personRecord, schema.typeParametersField, .uuid(makeList([])))

        // Instance: Person { name: "Alice", age: Some { value: _ } }
        let aliceAge = UUID()
        set(aliceAge, schema.recordField, .uuid(someRecord))

        let alice = UUID()
        set(alice, schema.recordField, .uuid(personRecord))
        set(alice, schema.nameField, .string("Alice"))
        set(alice, .uuid(ageField), .uuid(aliceAge))

        let root: Id = .uuid(makeList([.uuid(optionSum), .uuid(personRecord), .uuid(alice)]))
        return Editor(schema: schema, document: document, root: root, onChange: onChange)
    }
}
