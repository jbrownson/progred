import * as React from "react"
import { maybe, nothing } from "../../lib/Maybe"
import type { PlaceholderEditorActiveState, PlaceholderEditorState } from "../render/DEditors"
import type { EditorDescend } from "../render/DContext"
import type { ListInsertionPoint } from "../render/DLayout"
import { attachListInsertionPoint, detachListInsertionPoint, requestFocusActiveEditor, requestNextTabStopFromDescendChildFromActiveElement } from "../editor/EditorFocus"
import type { EditorCommands } from "../editor/EditorCommands"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"

export function ListInsertionEditorComponent(props: {insertionPoint: ListInsertionPoint, label: string, active: boolean, setActive: (active: boolean) => void, insertionIndex: number, descend?: EditorDescend, runE: (f: () => void) => void}) {
  const editorState = React.useRef<PlaceholderEditorState>({})
  const activeState = React.useRef<PlaceholderEditorActiveState | undefined>(undefined)
  const insertionPointSpan = React.useRef<HTMLSpanElement | null>(null)
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const setInsertionPointSpan = (element: HTMLSpanElement | null) => {
    if (insertionPointSpan.current) detachListInsertionPoint(insertionPointSpan.current)
    insertionPointSpan.current = element
    if (element) attachListInsertionPoint(element) }
  React.useEffect(() => () => {
    if (insertionPointSpan.current) detachListInsertionPoint(insertionPointSpan.current) }, [])
  const clearEditorState = () => {
    editorState.current.completionOpen = false
    editorState.current.value = ""
    editorState.current.itemSelection = nothing }
  const setActive = (active: boolean) => {
    if (active) activeState.current = activeState.current || {entries: props.insertionPoint.entries(), editorState: editorState.current}
    else activeState.current = undefined
    props.setActive(active) }
  const cancelWithRefocus = () => {
    requestFocusActiveEditor()
    clearEditorState()
    setActive(false) }
  const editorCommands: EditorCommands = {
    ...props.insertionPoint.editorCommands,
    keyDown: e => e.key === "Backspace" || e.key === "Delete" ? () => {
      e.preventDefault()
      e.stopPropagation()
      cancelWithRefocus() } : maybe(props.insertionPoint.editorCommands.keyDown, () => nothing, keyDown => keyDown(e)) }
  if (!props.active) return <span
    className="listInsertionPoint"
    tabIndex={0}
    onFocus={() => setActive(true)}
    onMouseDown={e => e.stopPropagation()}
    onClick={e => { e.stopPropagation(); setActive(true) }}
    ref={setInsertionPointSpan}>
      {props.label}<span className="listInsertionPointHitbox" />
    </span>
  activeState.current = activeState.current || {entries: props.insertionPoint.entries(), editorState: editorState.current}
  return <PlaceholderInputComponent
    activeState={activeState.current}
    placeholder="item"
    editorCommands={editorCommands}
    descend={props.descend}
    runE={props.runE}
    closeCompletion={() => {
      clearEditorState()
      forceUpdate() }}
    cancel={() => {
      clearEditorState()
      setActive(false) }}
    blur={() => {
      clearEditorState()
      setActive(false) }}
    commit={(action, e) => {
      e.preventDefault()
      e.stopPropagation()
      props.runE(() => {
        requestNextTabStopFromDescendChildFromActiveElement(props.insertionIndex)
        clearEditorState()
        setActive(false)
        action() })}}
  />
}
