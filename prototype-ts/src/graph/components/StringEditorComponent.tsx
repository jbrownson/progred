import * as React from "react"
import { mapMaybe } from "../../lib/Maybe"
import { noop } from "../../lib/noop"
import { cursorFromD, descendFromD } from "../cursor/cursorFromD"
import { StringEditor } from "../render/D"
import { sidFromString } from "../model/ID"
import { attachEditorCommands, detachEditorCommands, editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"

export class StringEditorComponent extends React.Component<{stringEditor: StringEditor, editorCommands: EditorCommands, runE: (f: () => void) => void}, {}> {
  textArea: HTMLTextAreaElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    let keyDownAction = editorKeyDownAction(this.props.editorCommands, e)
    if (keyDownAction) { this.props.runE(keyDownAction); return }
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) stopPropagationForTextInputs(e) }
  attachEditorCommands() {
    if (this.textArea) {
      attachEditorCommands(this.textArea, this.props.editorCommands)
      mapMaybe(cursorFromD(this.props.stringEditor), cursor => attachEditorFocus(this.textArea!, {cursor, descend: descendFromD(this.props.stringEditor)})) }}
  onScroll() { noop() }
  render() {
    return <textarea
      className="string i"
      rows={1}
      wrap="off"
      spellCheck={false}
      onChange={e => { if (this.props.stringEditor.writable)
        this.props.runE(() => mapMaybe(this.props.editorCommands.commit, commit => { if (this.textArea) commit(sidFromString(this.textArea.value)) }))}}
      value={this.props.stringEditor.string}
      onClick={e => e.stopPropagation() }
      onKeyDown={e => this.onKeyDown(e)}
      ref={input => { this.textArea = input }} /> }
  componentDidMount() { this.attachEditorCommands() }
  componentDidUpdate() { this.attachEditorCommands() }
  componentWillUnmount() { if (this.textArea) { detachEditorCommands(this.textArea); detachEditorFocus(this.textArea) } } }
