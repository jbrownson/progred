import { bindMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { environment } from "../Environment"

const editorFocusKey = Symbol("editorFocus")

type EditorFocusElement = HTMLElement & {[editorFocusKey]?: Cursor}

export function attachEditorFocus(element: HTMLElement, cursor: Cursor) {
  (element as EditorFocusElement)[editorFocusKey] = cursor
}

export function detachEditorFocus(element: HTMLElement) {
  delete (element as EditorFocusElement)[editorFocusKey]
}

export function cursorForActiveElement(): Maybe<Cursor> {
  let element = document.activeElement
  return element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey] : nothing
}

export function activeSelectionCursor(): Maybe<Cursor> {
  return cursorForActiveElement() || bindMaybe(environment().selection, selection => selection.cursor)
}
