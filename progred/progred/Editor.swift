import SwiftUI

@Observable
class Editor {
    let schema: Schema
    var document: MutGid
    var root: Id

    init(schema: Schema) {
        self.schema = schema
        self.document = MutGid()
        self.root = schema.library
    }

    var gid: StackedGid<MutGid, MutGid> {
        StackedGid(top: document, bottom: schema.gid)
    }

    func set(entity: UUID, label: Id, value: Id) {
        document.set(entity: entity, label: label, value: value)
    }

    func delete(entity: UUID, label: Id) {
        document.delete(entity: entity, label: label)
    }

    static func withSampleDocument() -> Editor {
        let schema = Schema.bootstrap()
        let editor = Editor(schema: schema)

        // Create a simple Record type "Person" with fields "name" and "age"
        let personRecord = UUID()
        let ageField = UUID()

        // Person is a Record
        editor.set(entity: personRecord, label: schema.recordField, value: .uuid(schema.recordRecord.asUUID!))
        editor.set(entity: personRecord, label: schema.nameField, value: .string("Person"))

        // age field
        editor.set(entity: ageField, label: schema.recordField, value: .uuid(schema.fieldRecord.asUUID!))
        editor.set(entity: ageField, label: schema.nameField, value: .string("age"))
        editor.set(entity: ageField, label: schema.typeExpressionField, value: schema.numberRecord)

        // Person's fields list: [name (reuse schema's), age]
        let empty = UUID()
        editor.set(entity: empty, label: schema.recordField, value: .uuid(schema.emptyRecord.asUUID!))

        let cons2 = UUID()
        editor.set(entity: cons2, label: schema.recordField, value: .uuid(schema.consRecord.asUUID!))
        editor.set(entity: cons2, label: schema.headField, value: .uuid(ageField))
        editor.set(entity: cons2, label: schema.tailField, value: .uuid(empty))

        let cons1 = UUID()
        editor.set(entity: cons1, label: schema.recordField, value: .uuid(schema.consRecord.asUUID!))
        editor.set(entity: cons1, label: schema.headField, value: schema.nameField)
        editor.set(entity: cons1, label: schema.tailField, value: .uuid(cons2))

        editor.set(entity: personRecord, label: schema.fieldsField, value: .uuid(cons1))

        // Empty type parameters
        let emptyParams = UUID()
        editor.set(entity: emptyParams, label: schema.recordField, value: .uuid(schema.emptyRecord.asUUID!))
        editor.set(entity: personRecord, label: schema.typeParametersField, value: .uuid(emptyParams))

        // Set root to Person so we view the document
        editor.root = .uuid(personRecord)

        return editor
    }
}
