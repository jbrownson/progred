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

    func handleDelete(path: Path?) {
        guard let path, let (parent, field) = path.pop(),
              case .uuid(let uuid) = parent.node(in: gid, root: root)
        else { return }

        // List splice: if parent is a cons cell, replace its tail with the next tail
        if field == schema.tailField,
           let child = gid.get(entity: .uuid(uuid), label: field),
           let grandchildTail = gid.get(entity: child, label: schema.tailField) {
            set(entity: uuid, label: field, value: grandchildTail)
        } else {
            delete(entity: uuid, label: field)
        }
    }

    func handleSet(path: Path?, value: Id) {
        guard let path, let (parent, field) = path.pop(),
              case .uuid(let uuid) = parent.node(in: gid, root: root)
        else { return }
        set(entity: uuid, label: field, value: value)
    }

    static func withSampleDocument() -> Editor {
        let schema = Schema.bootstrap()
        let editor = Editor(schema: schema)

        let personRecord = UUID()
        let ageField = UUID()

        editor.set(entity: personRecord, label: schema.recordField, value: schema.recordRecord)
        editor.set(entity: personRecord, label: schema.nameField, value: .string("Person"))

        editor.set(entity: ageField, label: schema.recordField, value: schema.fieldRecord)
        editor.set(entity: ageField, label: schema.nameField, value: .string("age"))
        editor.set(entity: ageField, label: schema.typeExpressionField, value: schema.numberRecord)

        let empty = UUID()
        editor.set(entity: empty, label: schema.recordField, value: schema.emptyRecord)

        let cons2 = UUID()
        editor.set(entity: cons2, label: schema.recordField, value: schema.consRecord)
        editor.set(entity: cons2, label: schema.headField, value: .uuid(ageField))
        editor.set(entity: cons2, label: schema.tailField, value: .uuid(empty))

        let cons1 = UUID()
        editor.set(entity: cons1, label: schema.recordField, value: schema.consRecord)
        editor.set(entity: cons1, label: schema.headField, value: schema.nameField)
        editor.set(entity: cons1, label: schema.tailField, value: .uuid(cons2))

        editor.set(entity: personRecord, label: schema.fieldsField, value: .uuid(cons1))

        let emptyParams = UUID()
        editor.set(entity: emptyParams, label: schema.recordField, value: schema.emptyRecord)
        editor.set(entity: personRecord, label: schema.typeParametersField, value: .uuid(emptyParams))

        editor.root = .uuid(personRecord)
        return editor
    }
}
