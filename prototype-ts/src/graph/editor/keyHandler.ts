import { maybe, nothing } from "../../lib/Maybe"
import { commitToActiveElement, editorCommandsForActiveElement, editorKeyDownAction } from "./EditorCommands"
import { focusChildEditor, focusFirstEditor, focusNextTabStop, focusParentEditor, focusSiblingEditor } from "./EditorFocus"

export type KeyHandler = (e: KeyboardEvent, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, runE))) }

export function deleteKeyHandler(e: KeyboardEvent, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Delete":
      return runE(() => {
        let committed = commitToActiveElement(nothing)
        if (committed) {
          e.stopPropagation()
          e.preventDefault() }
        return committed })
    case "Backspace":
      return runE(() => {
        let committed = commitToActiveElement(nothing)
        if (committed) {
          e.stopPropagation()
          e.preventDefault() }
        return committed })}
  return false }

export function activeEditorKeyHandler(e: KeyboardEvent, runE: <A>(f: () => A) => A): boolean {
  let keyDownAction = editorKeyDownAction(editorCommandsForActiveElement(), e)
  return maybe(keyDownAction, () => false, action => runE(() => {
    action()
    return true })) }

export function arrowNavKeyHandler(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault()
      return focusParentEditor()
    case "ArrowRight":
      e.preventDefault()
      return focusChildEditor() || focusFirstEditor()
    case "ArrowDown":
      e.preventDefault()
      return focusSiblingEditor(1) || focusFirstEditor()
    case "ArrowUp":
      e.preventDefault()
      return focusSiblingEditor(-1)}
  return false }

export function navKeyHandler(e: KeyboardEvent): boolean {
  switch (e.key) {
    case "Tab": {
      e.preventDefault()
      focusNextTabStop(e.shiftKey)
      return true }
    case "Escape": {
      e.preventDefault()
      return false }}
  return false }

export let defaultKeyHandler: KeyHandler = composedKeyHandler(activeEditorKeyHandler, deleteKeyHandler, navKeyHandler, arrowNavKeyHandler)
