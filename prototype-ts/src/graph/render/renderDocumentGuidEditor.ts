import { Cursor } from "../cursor/Cursor"
import { SourceID, SourceType } from "../Environment"
import { copyResultForID } from "../editor/Copy"
import type { EditorCommands } from "../editor/EditorCommands"
import { guidFromID, ID } from "../model/ID"
import type { D } from "./DContext"
import { guidEditor, supportsUnderselection } from "./DEditors"
import { renderField } from "./renderField"

export function editorCommands(cursor: Cursor, id: ID): EditorCommands {
  return {
    copy: () => ({referenceID: id, copyResult: copyResultForID(id)}) }}

export function renderDocumentGuidEditor(cursor: Cursor, sourceID: SourceID, d: D, rootEditorCommands: EditorCommands = {}): D {
  let guid = guidFromID(sourceID.id)
  return sourceID.source.source === SourceType.DocumentType && guid !== undefined
    ? supportsUnderselection(cursor, guid, guidEditor(cursor, guid, d, true, editorCommands(cursor, guid), rootEditorCommands), missingLabel => renderField(cursor, guid, missingLabel))
    : d }
