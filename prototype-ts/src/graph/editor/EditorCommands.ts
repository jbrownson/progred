import { Maybe, nothing } from "../../lib/Maybe"

export type EditorCommands = {}

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
