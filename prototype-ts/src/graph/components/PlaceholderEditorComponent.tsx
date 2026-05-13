import * as React from "react"
import { mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { cursorFromD, descendFromD } from "../cursor/cursorFromD"
import { createD, PlaceholderEditor, PlaceholderEditorActiveState, PlaceholderEditorState } from "../render/D"
import { doTab } from "../editor/keyHandler"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"
import { attachEditorCommands, detachEditorCommands, editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus, requestFocusForCursor } from "../editor/EditorFocus"
import { handleFocusEvent } from "../editor/ignoreFocusEvents"

type PlaceholderEditorComponentState = {active: boolean}

export class PlaceholderEditorComponent extends React.Component<{placeholderEditor: PlaceholderEditor, editorCommands: EditorCommands, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, PlaceholderEditorComponentState> {
  state = {active: false}
  span: HTMLSpanElement | null
  placeholderInput: PlaceholderInputComponent | null
  editorState: PlaceholderEditorState = {}
  close(activeState: PlaceholderEditorActiveState) {
    activeState.editorState.completionOpen = false
    activeState.editorState.value = ""
    activeState.editorState.itemSelection = nothing
    this.forceUpdate() }
  activeState() {
    return this.props.placeholderEditor.activeState || (this.state.active ? {entries: this.props.placeholderEditor.entries, editorState: this.editorState} : nothing) }
  activate() { this.setState({active: true}) }
  selectAndActivate() { this.activate() }
  deactivate(e?: React.FocusEvent<HTMLInputElement>) {
    if (e) e.currentTarget.value = ""
    this.editorState = {}
    this.setState({active: false}) }
  attachInactiveEditor() {
    if (this.span) {
      attachEditorCommands(this.span, this.props.editorCommands)
      mapMaybe(cursorFromD(this.props.placeholderEditor), cursor => attachEditorFocus(this.span!, {cursor, descend: descendFromD(this.props.placeholderEditor), activate: () => this.activate()})) }}
  onScroll() { if (this.placeholderInput) this.placeholderInput.onScroll() }
  render() {
    return maybe(this.activeState(), () =>
      <span
        className="uneditable"
        tabIndex={0}
        onFocus={() => handleFocusEvent(() => this.props.runE(() => this.selectAndActivate()))}
        onMouseDown={e => e.stopPropagation()}
        onClick={e => { e.stopPropagation(); this.props.runE(() => this.selectAndActivate()) }}
        ref={span => { if (this.span && this.span !== span) { detachEditorCommands(this.span); detachEditorFocus(this.span) } this.span = span }} >
        {this.props.placeholderEditor.name}
      </span>,
    activeState => {
      let runEditorKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => maybe(editorKeyDownAction(this.props.editorCommands, e),
        () => stopPropagationForTextInputs(e),
        action => this.props.runE(action))
      let runEntryListKeyDown = (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => mapMaybe(editorKeyDownAction(this.props.editorCommands, e), action =>
        this.props.runE(() => {
          commitActionIfSomethingToCommit()
          action() }))
      return <PlaceholderInputComponent
        ref={placeholderInput => { this.placeholderInput = placeholderInput }}
        activeState={activeState}
        placeholder={this.props.placeholderEditor.name}
        editorCommands={this.props.editorCommands}
        cursor={cursorFromD(this.props.placeholderEditor)}
        descend={descendFromD(this.props.placeholderEditor)}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE}
        closeCompletion={() => this.close(activeState)}
        cancel={() => this.props.runE(() => this.deactivate())}
        blur={e => this.props.runE(() => this.deactivate(e))}
        commit={(action, e) => {
          e.preventDefault()
          e.stopPropagation()
          this.props.runE(() => {
            const cursor = cursorFromD(this.props.placeholderEditor)
            action()
            let {rootDescend, viewsDescend} = createD()
            if (!doTab(false, rootDescend, viewsDescend, cursor))
              mapMaybe(cursor, requestFocusForCursor) })}}
        keyDown={runEditorKeyDown}
        entryListKeyDown={runEntryListKeyDown}
      /> })
  }
  componentDidMount() { this.attachInactiveEditor() }
  componentDidUpdate() { this.attachInactiveEditor() }
  componentWillUnmount() { if (this.span) { detachEditorCommands(this.span); detachEditorFocus(this.span) } }
}
