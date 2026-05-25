import { edges, SourceID, SourceType } from "../Environment"
import { copyResultForID } from "../editor/Copy"
import type { EditorCommands } from "../editor/EditorCommands"
import { Edge } from "../model/Edge"
import { guidFromID, ID } from "../model/ID"
import type { D } from "./DContext"
import { guidEditor, supportsUnderselection } from "./DEditors"
import { renderField } from "./renderField"

export function editorCommands(id: ID): EditorCommands {
  return {
    copy: () => ({referenceID: id, copyResult: copyResultForID(id)}) }}

export function renderDocumentGuidEditor(edge: Edge, sourceID: SourceID, d: D, rootEditorCommands: EditorCommands = {}): D {
  let guid = guidFromID(sourceID.id)
  if (guid === undefined) return d
  let editor = guidEditor(edge, guid, d, true, editorCommands(guid), rootEditorCommands)
  let source = edges(sourceID.id)?.source || sourceID.source
  return source.source === SourceType.DocumentType
    ? supportsUnderselection(edge, guid, editor, missingLabel => renderField(guid, missingLabel))
    : editor }
