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

private func typeMatches(_ record: Id?, _ expectedType: Id?, substitution: Substitution, editor: Editor) -> Bool {
    guard let expectedType, let record else { return true }
    return admits(record, expectedType, substitution, gid: editor.gid, schema: editor.schema) != false
}

private func dataEntries(_ named: [NamedEntity], editor: Editor, commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution) -> [PlaceholderEntry] {
    named.map { entity in
        PlaceholderEntry(
            display: entity.name,
            disambiguation: entity.record.flatMap { editor.name(of: $0) },
            action: { editor in commit(editor, entity.id) },
            matching: typeMatches(entity.record, expectedType, substitution: substitution, editor: editor),
            magic: false)
    }
}

private func newEntries(_ named: [NamedEntity], editor: Editor, commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution) -> [PlaceholderEntry] {
    named.compactMap { entity in
        guard entity.record == editor.schema.recordRecord,
              entity.id != editor.schema.stringRecord,
              entity.id != editor.schema.numberRecord
        else { return nil }
        return PlaceholderEntry(
            display: "new \(entity.name)",
            disambiguation: nil,
            action: { editor in
                let uuid = UUID()
                editor.commit(entity: uuid, label: editor.schema.recordField, value: entity.id)
                commit(editor, .uuid(uuid))
            },
            matching: typeMatches(entity.id, expectedType, substitution: substitution, editor: editor),
            magic: false)
    }
}

private func magicEntries(needle: String, editor: Editor, commit: @escaping (Editor, Id) -> Void, expectedType: Id?, substitution: Substitution) -> [PlaceholderEntry] {
    [
        Double(needle).map { n in
            PlaceholderEntry(display: needle, disambiguation: nil,
                action: { editor in commit(editor, .number(n)) },
                matching: typeMatches(editor.schema.numberRecord, expectedType, substitution: substitution, editor: editor),
                magic: true)
        },
        PlaceholderEntry(display: "\"\(needle)\"", disambiguation: nil,
            action: { editor in commit(editor, .string(needle)) },
            matching: typeMatches(editor.schema.stringRecord, expectedType, substitution: substitution, editor: editor),
            magic: true),
    ].compactMap { $0 }
}

func buildEntries(editor: Editor, commit: @escaping (Editor, Id) -> Void, needle: String, expectedType: Id?, substitution: Substitution) -> [PlaceholderEntry] {
    let named = namedEntities(editor: editor).values
        .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    return dataEntries(named, editor: editor, commit: commit, expectedType: expectedType, substitution: substitution)
        + newEntries(named, editor: editor, commit: commit, expectedType: expectedType, substitution: substitution)
        + magicEntries(needle: needle, editor: editor, commit: commit, expectedType: expectedType, substitution: substitution)
}
