import { bindMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { cursorsEqual } from "../cursor/Cursor"
import { environment } from "../Environment"
import { focus } from "./ignoreFocusEvents"

const editorFocusKey = Symbol("editorFocus")

type EditorFocus = {
  cursor: Cursor
  focusWhenSelected?: boolean
}

type EditorFocusElement = HTMLElement & {[editorFocusKey]?: EditorFocus}

export function attachEditorFocus(element: HTMLElement, focus: EditorFocus) {
  (element as EditorFocusElement)[editorFocusKey] = focus
}

export function detachEditorFocus(element: HTMLElement) {
  delete (element as EditorFocusElement)[editorFocusKey]
}

export function cursorForActiveElement(): Maybe<Cursor> {
  let element = document.activeElement
  return element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey]?.cursor : nothing
}

export function activeSelectionCursor(): Maybe<Cursor> {
  return cursorForActiveElement() || bindMaybe(environment().selection, selection => selection.cursor)
}

export function focusEditorForCursor(root: HTMLElement, cursor: Cursor): boolean {
  for (let element of [root, ...Array.from(root.querySelectorAll("*"))]) {
    let editorFocus = element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey] : nothing
    if (editorFocus !== nothing && editorFocus.focusWhenSelected !== false && cursorsEqual(editorFocus.cursor, cursor)) {
      focus(element as HTMLElement)
      return true }}
  return false
}
