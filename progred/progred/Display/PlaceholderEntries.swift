import Foundation
import HashTreeCollections

struct PlaceholderEntry {
    let display: String
    let disambiguation: String?
    let action: (Editor) -> Void
    let matching: Bool
    let magic: Bool
}

struct NamedEntity {
    let id: Id
    let name: String
    let record: Id?
}

func namedEntities(editor: Editor) -> [Id: NamedEntity] {
    let recordField = editor.schema.recordField

    return Dictionary(
        (Array(editor.schema.gid.data.keys) + Array(editor.document.data.keys))
            .map { Id.uuid($0) }
            .compactMap { id in editor.name(of: id).map { (id, $0) } }
            .map { id, name in (id, NamedEntity(id: id, name: name, record: editor.gid.get(entity: id, label: recordField))) },
        uniquingKeysWith: { _, latest in latest })
}

private func dataEntries(_ named: [NamedEntity], editor: Editor, commit: @escaping Commit) -> [PlaceholderEntry] {
    named.map { entity in
        PlaceholderEntry(
            display: entity.name,
            disambiguation: entity.record.flatMap { editor.name(of: $0) },
            action: { editor in commit(editor, entity.id) },
            matching: true,
            magic: false)
    }
}

private func newEntries(_ named: [NamedEntity], schema: Schema, commit: @escaping Commit) -> [PlaceholderEntry] {
    named.compactMap { entity in
        guard entity.record == schema.recordRecord || entity.record == schema.sumRecord else { return nil }
        return PlaceholderEntry(
            display: "new \(entity.name)",
            disambiguation: nil,
            action: { editor in
                let uuid = UUID()
                editor.commit(entity: uuid, label: schema.recordField, value: entity.id)
                commit(editor, .uuid(uuid))
            },
            matching: true,
            magic: false)
    }
}

private func magicEntries(needle: String, commit: @escaping Commit) -> [PlaceholderEntry] {
    [
        Double(needle).map { n in
            PlaceholderEntry(display: needle, disambiguation: nil,
                action: { editor in commit(editor, .number(n)) }, matching: true, magic: true)
        },
        PlaceholderEntry(display: "\"\(needle)\"", disambiguation: nil,
            action: { editor in commit(editor, .string(needle)) }, matching: true, magic: true),
    ].compactMap { $0 }
}

func buildEntries(editor: Editor, commit: @escaping Commit, needle: String) -> [PlaceholderEntry] {
    let named = namedEntities(editor: editor).values
        .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    return dataEntries(named, editor: editor, commit: commit)
        + newEntries(named, schema: editor.schema, commit: commit)
        + magicEntries(needle: needle, commit: commit)
}
