import { Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { cursorsEqual } from "../cursor/Cursor"
import type { Descend } from "../render/D"
import { focus } from "./ignoreFocusEvents"

const editorFocusKey = Symbol("editorFocus")
let pendingFocusCursor: Maybe<Cursor> = nothing

type EditorFocus = {
  cursor: Cursor
  descend?: Descend
  activate?: () => void
  focusWhenSelected?: boolean
}

type EditorFocusElement = HTMLElement & {[editorFocusKey]?: EditorFocus}

function editorFocusForElement(element: Maybe<Element>): Maybe<EditorFocus> {
  return element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey] : nothing
}

export function attachEditorFocus(element: HTMLElement, focus: EditorFocus) {
  (element as EditorFocusElement)[editorFocusKey] = focus
}

export function detachEditorFocus(element: HTMLElement) {
  delete (element as EditorFocusElement)[editorFocusKey]
}

export function descendForActiveElement(): Maybe<Descend> {
  return editorFocusForActiveElement()?.descend
}

export function editorFocusForActiveElement(): Maybe<EditorFocus> {
  return editorFocusForElement(document.activeElement)
}

export function focusEditorForCursor(root: HTMLElement, cursor: Cursor): boolean {
  for (let element of [root, ...Array.from(root.querySelectorAll("*"))]) {
    let editorFocus = element instanceof HTMLElement ? (element as EditorFocusElement)[editorFocusKey] : nothing
    if (editorFocus !== nothing && editorFocus.focusWhenSelected !== false && cursorsEqual(editorFocus.cursor, cursor)) {
      pendingFocusCursor = nothing
      focus(element as HTMLElement)
      editorFocus.activate?.()
      return true }}
  return false
}

export function requestFocusForCursor(cursor: Cursor) {
  pendingFocusCursor = cursor
}

export function focusPendingEditor(root: HTMLElement): boolean {
  if (pendingFocusCursor === nothing) return false
  const cursor = pendingFocusCursor
  const focused = focusEditorForCursor(root, cursor)
  if (focused) pendingFocusCursor = nothing
  return focused
}

export function focusEditorForDescend(descend: Descend): boolean {
  for (let element of Array.from(document.querySelectorAll("*"))) {
    let editorFocus = editorFocusForElement(element)
    if (editorFocus !== nothing && editorFocus.focusWhenSelected !== false && editorFocus.descend === descend) {
      pendingFocusCursor = nothing
      focus(element as HTMLElement)
      editorFocus.activate?.()
      return true }}
  requestFocusForCursor(descend.cursor)
  return false
}
