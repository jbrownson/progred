import * as React from "react"
import { mapMaybe, nothing } from "../../lib/Maybe"
import { noop } from "../../lib/noop"
import { cursorFromD } from "../cursor/cursorFromD"
import { StringEditor } from "../render/D"
import { environment } from "../Environment"
import { sidFromString } from "../model/ID"
import { attachEditorCommands, detachEditorCommands } from "../editor/EditorCommands"
import { blur, focus, handleFocusEvent } from "../editor/ignoreFocusEvents"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"

export class StringEditorComponent extends React.Component<{stringEditor: StringEditor, runE: (f: () => void) => void}, {}> {
  textArea: HTMLTextAreaElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) stopPropagationForTextInputs(e) }
  focusIfSelected() { if (this.textArea) (this.props.stringEditor.stringEditorSelectedState ? focus : blur)(this.textArea) }
  attachEditorCommands() {
    if (this.textArea) attachEditorCommands(this.textArea, this.props.stringEditor.editorCommands) }
  onScroll() { noop() }
  render() {
    return <textarea
      className="string i"
      rows={1}
      wrap="off"
      spellCheck={false}
      onChange={e => { if (this.props.stringEditor.stringEditorSelectedState && this.props.stringEditor.stringEditorSelectedState.writable)
        this.props.runE(() => mapMaybe(this.props.stringEditor.editorCommands.commit, commit => { if (this.textArea) commit(sidFromString(this.textArea.value)) }))}}
      value={this.props.stringEditor.string}
      onFocus={e => handleFocusEvent(() => this.props.runE(() => mapMaybe(cursorFromD(this.props.stringEditor), cursor => environment().selection = {cursor})))}
      onBlur={e => handleFocusEvent(() => this.props.runE(() => environment().selection = nothing))}
      onClick={e => e.stopPropagation() }
      onKeyDown={e => this.onKeyDown(e)}
      ref={input => { this.textArea = input }} /> }
  componentDidMount() { this.focusIfSelected(); this.attachEditorCommands() }
  componentDidUpdate() { this.focusIfSelected(); this.attachEditorCommands() }
  componentWillUnmount() { if (this.textArea) detachEditorCommands(this.textArea) } }
