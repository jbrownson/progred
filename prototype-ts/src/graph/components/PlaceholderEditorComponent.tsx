import * as React from "react"
import { mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import type { EditorDescend } from "../render/ProjectionContext"
import type { PlaceholderEditor, PlaceholderEditorActiveState, PlaceholderEditorState } from "../render/ProjectionEditors"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"
import { editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { requestNextTabStopFromCursor } from "../editor/EditorFocus"
import { handleFocusEvent } from "../editor/ignoreFocusEvents"
import { useEditorAttachment } from "./useEditorAttachment"

export function PlaceholderEditorComponent(props: {placeholderEditor: PlaceholderEditor, editorCommands: EditorCommands, cursor?: Cursor, descend?: EditorDescend, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}) {
  const [active, setActive] = React.useState(false)
  const span = React.useRef<HTMLSpanElement | null>(null)
  const editorState = React.useRef<PlaceholderEditorState>({})
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const activate = () => setActive(true)
  const close = (activeState: PlaceholderEditorActiveState) => {
    activeState.editorState.completionOpen = false
    activeState.editorState.value = ""
    activeState.editorState.itemSelection = nothing
    forceUpdate() }
  const deactivate = (e?: React.FocusEvent<HTMLInputElement>) => {
    if (e) e.currentTarget.value = ""
    editorState.current = {}
    setActive(false) }
  useEditorAttachment(span, props.editorCommands, {cursor: props.cursor, descend: props.descend, activate, tabStop: true})
  return maybe(props.placeholderEditor.activeState || (active ? {entries: props.placeholderEditor.entries, editorState: editorState.current} : nothing), () =>
    <span
      className="uneditable"
      tabIndex={0}
      onFocus={() => handleFocusEvent(() => props.runE(activate))}
      onMouseDown={e => e.stopPropagation()}
      onClick={e => { e.stopPropagation(); props.runE(activate) }}
      ref={span} >
      {props.placeholderEditor.name}
    </span>,
  activeState => {
    let runEditorKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => maybe(editorKeyDownAction(props.editorCommands, e),
      () => stopPropagationForTextInputs(e),
      action => props.runE(action))
    let runEntryListKeyDown = (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => mapMaybe(editorKeyDownAction(props.editorCommands, e), action =>
      props.runE(() => {
        commitActionIfSomethingToCommit()
        action() }))
    return <PlaceholderInputComponent
      activeState={activeState}
      placeholder={props.placeholderEditor.name}
      editorCommands={props.editorCommands}
      cursor={props.cursor}
      descend={props.descend}
      tabStop={true}
      scrollParent={props.scrollParent}
      runE={props.runE}
      closeCompletion={() => close(activeState)}
      cancel={() => props.runE(() => deactivate())}
      blur={e => props.runE(() => deactivate(e))}
      commit={(action, e) => {
        e.preventDefault()
        e.stopPropagation()
        props.runE(() => {
          action()
          mapMaybe(props.cursor, cursor => requestNextTabStopFromCursor(cursor)) })}}
      keyDown={runEditorKeyDown}
      entryListKeyDown={runEntryListKeyDown}
    /> })
}
