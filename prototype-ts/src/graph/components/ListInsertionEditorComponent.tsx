import * as React from "react"
import { nothing } from "../../lib/Maybe"
import { ListInsertionPoint, PlaceholderEditorSelectedState, PlaceholderEditorState } from "../render/D"
import { handleFocusEvent } from "../editor/ignoreFocusEvents"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"
import { environment } from "../Environment"

export class ListInsertionEditorComponent extends React.Component<{insertionPoint: ListInsertionPoint, label: string, active: boolean, setActive: (active: boolean) => void, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  placeholderInput: PlaceholderInputComponent | null
  editorState: PlaceholderEditorState = {}
  selectedState(): PlaceholderEditorSelectedState { return {entries: this.props.insertionPoint.entries, editorState: this.editorState} }
  close() {
    this.editorState.completionOpen = false
    this.editorState.value = ""
    this.editorState.itemSelection = nothing
    this.forceUpdate() }
  activate() { this.props.runE(() => {
    environment().selection = nothing
    this.props.setActive(true) }) }
  deactivate() {
    this.editorState.completionOpen = false
    this.editorState.value = ""
    this.editorState.itemSelection = nothing
    this.props.setActive(false) }
  onScroll() { if (this.placeholderInput) this.placeholderInput.onScroll() }
  render() {
    if (!this.props.active) return <span
      className="listInsertionPoint"
      tabIndex={0}
      onFocus={e => handleFocusEvent(() => this.activate())}
      onMouseDown={e => e.stopPropagation()}
      onClick={e => { e.stopPropagation(); this.activate() }}>
        {this.props.label}<span className="listInsertionPointHitbox" />
      </span>
    return <PlaceholderInputComponent
      ref={placeholderInput => { this.placeholderInput = placeholderInput }}
      selectedState={this.selectedState()}
      placeholder="item"
      editorCommands={this.props.insertionPoint.editorCommands}
      scrollParent={this.props.scrollParent}
      runE={this.props.runE}
      closeCompletion={() => this.close()}
      cancel={() => this.deactivate()}
      blur={() => this.deactivate()}
      commit={(action, e) => {
        e.preventDefault()
        e.stopPropagation()
        this.props.runE(() => {
          this.deactivate()
          action() })}}
    /> } }
