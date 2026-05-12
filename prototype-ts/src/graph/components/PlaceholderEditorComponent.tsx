import * as React from "react"
import { mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { cursorFromD } from "../cursor/cursorFromD"
import { createD, PlaceholderEditor, PlaceholderEditorSelectedState } from "../render/D"
import { environment } from "../Environment"
import { doTab } from "../editor/keyHandler"
import { appendToListCursor, insertAfterListElemCursor, selectionCursorBindMaybe, setCursorToEmptyList } from "../editor/listCursorActions"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"

export class PlaceholderEditorComponent extends React.Component<{placeholderEditor: PlaceholderEditor, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  placeholderInput: PlaceholderInputComponent | null
  close(selectedState: PlaceholderEditorSelectedState) {
    selectedState.editorState.completionOpen = false
    selectedState.editorState.value = ""
    selectedState.editorState.itemSelection = nothing
    this.forceUpdate() }
  onScroll() { if (this.placeholderInput) this.placeholderInput.onScroll() }
  render() {
    return maybe(this.props.placeholderEditor.selectedState, () =>
      <span className="uneditable" onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.placeholderEditor), cursor => environment().selection = {cursor})) }} >{this.props.placeholderEditor.name}</span>,
    selectedState =>
      <PlaceholderInputComponent
        ref={placeholderInput => { this.placeholderInput = placeholderInput }}
        selectedState={selectedState}
        placeholder={this.props.placeholderEditor.name}
        editorCommands={this.props.placeholderEditor.editorCommands}
        cursor={cursorFromD(this.props.placeholderEditor)}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE}
        closeCompletion={() => this.close(selectedState)}
        cancel={() => this.props.runE(() => environment().selection = nothing)}
        blur={e => this.props.runE(() => { e.currentTarget.value = ""; environment().selection = nothing })}
        commit={(action, e) => {
          e.preventDefault()
          e.stopPropagation()
          this.props.runE(() => {
            action()
            let {rootDescend, viewsDescend} = createD()
            doTab(false, rootDescend, viewsDescend) })}}
        keyDown={e => {
          switch (e.key) {
            case "[":
              this.props.runE(() => selectionCursorBindMaybe(cursor =>
                mapMaybe(setCursorToEmptyList(cursor), cursor => {
                  e.preventDefault()
                  e.stopPropagation()
                  maybe(appendToListCursor(cursor),
                    () => environment().selection = {cursor},
                    cursor => environment().selection = {cursor} )})))
              break
            case ",":
              if (!e.shiftKey || e.ctrlKey || e.altKey || e.metaKey)
                this.props.runE(() => {
                  mapMaybe(selectionCursorBindMaybe(cursor => insertAfterListElemCursor(cursor)), cursor => {
                    e.preventDefault()
                    e.stopPropagation()
                    environment().selection = {cursor} })})
              break
            default:
              stopPropagationForTextInputs(e) }}}
        entryListKeyDown={(e, commitActionIfSomethingToCommit) => {
          switch (e.key) {
            case "[":
              let value = selectedState.editorState.value
              if (value === nothing || value === "") {
                this.props.runE(() => selectionCursorBindMaybe(cursor =>
                  mapMaybe(setCursorToEmptyList(cursor), cursor => {
                    e.preventDefault()
                    e.stopPropagation()
                    maybe(appendToListCursor(cursor),
                      () => environment().selection = {cursor},
                      cursor => environment().selection = {cursor} )})))}
              break
            case ",":
              if (!e.shiftKey || e.ctrlKey || e.altKey || e.metaKey)
                this.props.runE(() => {
                  mapMaybe(selectionCursorBindMaybe(cursor => insertAfterListElemCursor(cursor)), cursor => {
                    e.preventDefault()
                    e.stopPropagation()
                    environment().selection = {cursor}
                    commitActionIfSomethingToCommit() })})
              break }}}
      /> )}
}
