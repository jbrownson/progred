import * as React from "react"
import { mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { Edge } from "../model/Edge"
import type { EditorDescend } from "../render/DContext"
import type { PlaceholderEditor, PlaceholderEditorActiveState, PlaceholderEditorState } from "../render/DEditors"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"
import { editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { requestNextTabStopFromActiveElement } from "../editor/EditorFocus"
import { useEditorAttachment } from "./useEditorAttachment"

export function PlaceholderEditorComponent(props: {placeholderEditor: PlaceholderEditor, editorCommands: EditorCommands, edge?: Edge, descend?: EditorDescend, runE: (f: () => void) => void}) {
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
  const deactivate = () => {
    editorState.current = {}
    setActive(false) }
  useEditorAttachment(span, props.editorCommands, {edge: props.edge, descend: props.descend, tabStop: true})
  return maybe(props.placeholderEditor.activeState || (active ? {entries: props.placeholderEditor.entries, editorState: editorState.current} : nothing), () =>
    <span
      className="uneditable"
      tabIndex={0}
      onFocus={() => activate()}
      onMouseDown={e => e.stopPropagation()}
      onClick={e => { e.stopPropagation(); activate() }}
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
      edge={props.edge}
      descend={props.descend}
      tabStop={true}
      runE={props.runE}
      closeCompletion={() => close(activeState)}
      cancel={() => deactivate()}
      blur={() => deactivate()}
      commit={(action, e) => {
        e.preventDefault()
        e.stopPropagation()
        props.runE(() => {
          requestNextTabStopFromActiveElement()
          action() })}}
      keyDown={runEditorKeyDown}
      entryListKeyDown={runEntryListKeyDown}
    /> })
}
