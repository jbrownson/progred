import { Maybe, maybe, nothing } from "../../lib/Maybe"
import type { ID } from "../model/ID"
import type { CopyResult } from "./Copy"

export type Commit = (id: Maybe<ID>) => void

export type EditorCommands = {
  copy?: () => {referenceID: ID, copyResult: CopyResult}
  commit?: Commit
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

export function commitToActiveElement(id: Maybe<ID>): boolean {
  return maybe(editorCommandsForActiveElement(), () => false, commands =>
    maybe(commands.commit, () => false, commit => {
      commit(id)
      return true })) }

export function commitIDToActiveElement(id: ID): boolean { return commitToActiveElement(id) }
