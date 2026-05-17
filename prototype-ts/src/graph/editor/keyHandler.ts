import { maybe, nothing } from "../../lib/Maybe"
import { editorCommandsForActiveElement, editorKeyDownAction } from "./EditorCommands"
import { deleteActiveElementWithRefocus } from "./commitWithFocus"
import { clearParentNavigationMemory, editorFocusForActiveElement, focusChildEditor, focusFirstEditor, focusNextTabStop, focusParentEditor, focusSiblingEditor } from "./EditorFocus"

export type KeyHandler = (e: KeyboardEvent, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, runE))) }

export function deleteKeyHandler(e: KeyboardEvent, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Delete":
      return runE(() => {
        let committed = deleteActiveElementWithRefocus()
        if (committed) {
          clearParentNavigationMemory()
          e.stopPropagation()
          e.preventDefault() }
        return committed })
    case "Backspace":
      return runE(() => {
        let committed = deleteActiveElementWithRefocus()
        if (committed) {
          clearParentNavigationMemory()
          e.stopPropagation()
          e.preventDefault() }
        return committed })}
  return false }

export function activeEditorKeyHandler(e: KeyboardEvent, runE: <A>(f: () => A) => A): boolean {
  let keyDownAction = editorKeyDownAction(editorCommandsForActiveElement(), e)
  return maybe(keyDownAction, () => false, action => runE(() => {
    clearParentNavigationMemory()
    action()
    return true })) }

function focusFirstEditorIfNothingFocused(): boolean {
  return editorFocusForActiveElement() === nothing && focusFirstEditor()
}

export function arrowNavKeyHandler(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "ArrowUp":
      e.preventDefault()
      focusParentEditor()
      return true
    case "ArrowDown":
      e.preventDefault()
      focusChildEditor() || focusFirstEditorIfNothingFocused()
      return true
    case "ArrowRight":
      e.preventDefault()
      focusSiblingEditor(1) || focusFirstEditorIfNothingFocused()
      return true
    case "ArrowLeft":
      e.preventDefault()
      focusSiblingEditor(-1)
      return true}
  return false }

export function navKeyHandler(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "Tab": {
      e.preventDefault()
      clearParentNavigationMemory()
      focusNextTabStop(e.shiftKey)
      return true }
    case "Escape": {
      e.preventDefault()
      clearParentNavigationMemory()
      return false }}
  return false }

export let defaultKeyHandler: KeyHandler = (e, runE) => {
  if (e.key !== "ArrowUp" && e.key !== "ArrowDown") clearParentNavigationMemory()
  return composedKeyHandler(activeEditorKeyHandler, deleteKeyHandler, navKeyHandler, arrowNavKeyHandler)(e, runE) }
