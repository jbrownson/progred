import { maybe, Maybe, nothing } from "../../lib/Maybe"
import { Descend } from "../render/D"
import { commitToActiveElement, editorCommandsForActiveElement, editorKeyDownAction } from "./EditorCommands"
import { focusChildEditor, focusFirstEditor, focusNextTabStop, focusParentEditor, focusSiblingEditor } from "./EditorFocus"

export type KeyHandler = (e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A) => boolean

function untilTrue(...fs: (() => boolean)[]): boolean { return fs.length > 0 && (fs[0]() || untilTrue(...fs.slice(1))) }

export function composedKeyHandler(...keyHandlers: KeyHandler[]): KeyHandler {
  return (e, rootDescend, viewsDescend, runE) => untilTrue(...keyHandlers.map(keyHandler => () => keyHandler(e, rootDescend, viewsDescend, runE))) }

export function deleteKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
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

export function activeEditorKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  let keyDownAction = editorKeyDownAction(editorCommandsForActiveElement(), e)
  return maybe(keyDownAction, () => false, action => runE(() => {
    action()
    return true })) }

export function arrowNavKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "ArrowLeft":
      e.preventDefault()
      return runE(focusParentEditor)
    case "ArrowRight":
      e.preventDefault()
      return runE(() => focusChildEditor() || focusFirstEditor())
    case "ArrowDown":
      e.preventDefault()
      return runE(() => focusSiblingEditor(1) || focusFirstEditor())
    case "ArrowUp":
      e.preventDefault()
      return runE(() => focusSiblingEditor(-1))}
  return false }

export function navKeyHandler(e: KeyboardEvent, rootDescend: Descend, viewsDescend: Maybe<Descend>, runE: <A>(f: () => A) => A): boolean {
  switch (e.key) {
    case "Tab": {
      e.preventDefault()
      runE(() => focusNextTabStop(e.shiftKey))
      return true }
    case "Escape": {
      e.preventDefault()
      return false }}
  return false }

export let defaultKeyHandler: KeyHandler = composedKeyHandler(activeEditorKeyHandler, deleteKeyHandler, navKeyHandler, arrowNavKeyHandler)
