import * as React from "react"
import { mapMaybe, nothing } from "../../lib/Maybe"
import { noop } from "../../lib/noop"
import { cursorFromD } from "../cursor/cursorFromD"
import { StringEditor } from "../render/D"
import { environment } from "../Environment"
import { sidFromString } from "../model/ID"
import { attachEditorCommands, detachEditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import { handleFocusEvent } from "../editor/ignoreFocusEvents"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"

export class StringEditorComponent extends React.Component<{stringEditor: StringEditor, runE: (f: () => void) => void}, {}> {
  textArea: HTMLTextAreaElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) stopPropagationForTextInputs(e) }
  attachEditorCommands() {
    if (this.textArea) {
      attachEditorCommands(this.textArea, this.props.stringEditor.editorCommands)
      mapMaybe(cursorFromD(this.props.stringEditor), cursor => attachEditorFocus(this.textArea!, {cursor})) }}
  onScroll() { noop() }
  render() {
    return <textarea
      className="string i"
      rows={1}
      wrap="off"
      spellCheck={false}
      onChange={e => { if (this.props.stringEditor.writable)
        this.props.runE(() => mapMaybe(this.props.stringEditor.editorCommands.commit, commit => { if (this.textArea) commit(sidFromString(this.textArea.value)) }))}}
      value={this.props.stringEditor.string}
      onFocus={e => handleFocusEvent(() => this.props.runE(() => mapMaybe(cursorFromD(this.props.stringEditor), cursor => environment().selection = {cursor})))}
      onBlur={e => handleFocusEvent(() => this.props.runE(() => environment().selection = nothing))}
      onClick={e => e.stopPropagation() }
      onKeyDown={e => this.onKeyDown(e)}
      ref={input => { this.textArea = input }} /> }
  componentDidMount() { this.attachEditorCommands() }
  componentDidUpdate() { this.attachEditorCommands() }
  componentWillUnmount() { if (this.textArea) { detachEditorCommands(this.textArea); detachEditorFocus(this.textArea) } } }
