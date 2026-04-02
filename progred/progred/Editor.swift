import Foundation
import HashTreeCollections
import Observation

@Observable
class Editor {
    let schema: Schema
    var document: MutGid
    var root: Id?

    init(schema: Schema) {
        self.schema = schema
        self.document = MutGid()
        self.root = schema.library
    }

    var gid: StackedGid<MutGid, ImmGid> {
        StackedGid(top: document, bottom: schema.gid)
    }

    func name(of entity: Id) -> String? {
        gid.get(entity: entity, label: schema.nameField)?.asString
    }

    func commit(entity: UUID, label: Id, value: Id?) {
        document.commit(entity: entity, label: label, value: value)
    }

    func commit(path: Path, value: Id?) {
        guard let (parent, field) = path.pop() else {
            if case .root = path.root { root = value }
            return
        }
        guard case .uuid(let uuid) = parent.node(in: gid, root: root) else { return }
        assert(document.data[uuid] != nil, "Attempted to modify non-document entity")
        commit(entity: uuid, label: field, value: value)
    }

    static func withSampleDocument() -> Editor {
        let schema = Schema.bootstrap()
        let editor = Editor(schema: schema)

        let personRecord = UUID()
        let ageField = UUID()

        editor.commit(entity: personRecord, label: schema.recordField, value: schema.recordRecord)
        editor.commit(entity: personRecord, label: schema.nameField, value: .string("Person"))

        editor.commit(entity: ageField, label: schema.recordField, value: schema.fieldRecord)
        editor.commit(entity: ageField, label: schema.nameField, value: .string("age"))

        let empty = UUID()
        editor.commit(entity: empty, label: schema.recordField, value: schema.emptyRecord)

        let cons2 = UUID()
        editor.commit(entity: cons2, label: schema.recordField, value: schema.consRecord)
        editor.commit(entity: cons2, label: schema.headField, value: .uuid(ageField))
        editor.commit(entity: cons2, label: schema.tailField, value: .uuid(empty))

        let cons1 = UUID()
        editor.commit(entity: cons1, label: schema.recordField, value: schema.consRecord)
        editor.commit(entity: cons1, label: schema.headField, value: schema.nameField)
        editor.commit(entity: cons1, label: schema.tailField, value: .uuid(cons2))

        editor.commit(entity: personRecord, label: schema.fieldsField, value: .uuid(cons1))

        let emptyParams = UUID()
        editor.commit(entity: emptyParams, label: schema.recordField, value: schema.emptyRecord)
        editor.commit(entity: personRecord, label: schema.typeParametersField, value: .uuid(emptyParams))

        editor.root = .uuid(personRecord)
        return editor
    }
}
