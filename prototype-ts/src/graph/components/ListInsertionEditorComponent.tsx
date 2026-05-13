import * as React from "react"
import { nothing } from "../../lib/Maybe"
import type { PlaceholderEditorActiveState, PlaceholderEditorState } from "../render/DEditors"
import type { ListInsertionPoint } from "../render/DLayout"
import { handleFocusEvent } from "../editor/ignoreFocusEvents"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"

export function ListInsertionEditorComponent(props: {insertionIndex: number, insertionPoint: ListInsertionPoint, label: string, active: boolean, setActive: (active: boolean) => void, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}) {
  const editorState = React.useRef<PlaceholderEditorState>({})
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const clearEditorState = () => {
    editorState.current.completionOpen = false
    editorState.current.value = ""
    editorState.current.itemSelection = nothing }
  const activeState: PlaceholderEditorActiveState = {entries: props.insertionPoint.entries, editorState: editorState.current}
  if (!props.active) return <span
    className="listInsertionPoint"
    data-list-insertion-index={props.insertionIndex}
    tabIndex={0}
    onFocus={() => handleFocusEvent(() => props.setActive(true))}
    onMouseDown={e => e.stopPropagation()}
    onClick={e => { e.stopPropagation(); props.setActive(true) }}>
      {props.label}<span className="listInsertionPointHitbox" />
    </span>
  return <PlaceholderInputComponent
    activeState={activeState}
    placeholder="item"
    editorCommands={props.insertionPoint.editorCommands}
    scrollParent={props.scrollParent}
    runE={props.runE}
    closeCompletion={() => {
      clearEditorState()
      forceUpdate() }}
    cancel={() => {
      clearEditorState()
      props.setActive(false) }}
    blur={() => {
      clearEditorState()
      props.setActive(false) }}
    commit={(action, e) => {
      e.preventDefault()
      e.stopPropagation()
      props.runE(() => {
        clearEditorState()
        props.setActive(false)
        action() })}}
  />
}
