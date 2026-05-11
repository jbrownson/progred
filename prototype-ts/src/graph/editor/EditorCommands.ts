import { Maybe, maybe, nothing } from "../../lib/Maybe"
import type { Descend } from "../render/D"
import type { ID } from "../model/ID"
import type { CopyResult } from "./Copy"

export type DeleteDirection = "forward" | "backward"

export type EditorCommands = {
  copy?: () => {referenceID: ID, copyResult: CopyResult}
  commitID?: (id: ID) => void
  delete?: (rootDescend: Descend, viewsDescend: Maybe<Descend>, direction: DeleteDirection) => boolean
}

const editorCommandsKey = Symbol("editorCommands")

type EditorCommandsElement = HTMLElement & {[editorCommandsKey]?: EditorCommands}

export function attachEditorCommands(element: HTMLElement, commands: EditorCommands) {
  (element as EditorCommandsElement)[editorCommandsKey] = commands
}

export function detachEditorCommands(element: HTMLElement) {
  delete (element as EditorCommandsElement)[editorCommandsKey]
}

export function editorCommandsForActiveElement(): Maybe<EditorCommands> {
  let element = document.activeElement
  return element instanceof HTMLElement ? (element as EditorCommandsElement)[editorCommandsKey] : nothing
}

export function commitIDToActiveElement(id: ID): boolean {
  return maybe(editorCommandsForActiveElement(), () => false, commands =>
    maybe(commands.commitID, () => false, commitID => {
      commitID(id)
      return true })) }

export function deleteActiveElement(rootDescend: Descend, viewsDescend: Maybe<Descend>, direction: DeleteDirection): boolean {
  return maybe(editorCommandsForActiveElement(), () => false, commands =>
    maybe(commands.delete, () => false, delete_ => delete_(rootDescend, viewsDescend, direction))) }
