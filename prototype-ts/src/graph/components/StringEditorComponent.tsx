import * as React from "react"
import { bindMaybe, mapMaybe, nothing } from "../../lib/Maybe"
import { noop } from "../../lib/noop"
import { cursorFromD } from "../cursorFromD"
import { StringEditor } from "../render/D"
import { environment, set } from "../Environment"
import { guidFromID, sidFromString } from "../ID"
import { blur, focus, handleFocusEvent } from "../editor/ignoreFocusEvents"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"

export class StringEditorComponent extends React.Component<{stringEditor: StringEditor, runE: (f: () => void) => void}, {}> {
  textArea: HTMLTextAreaElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) stopPropagationForTextInputs(e) }
  focusIfSelected() { if (this.textArea) (this.props.stringEditor.stringEditorSelectedState ? focus : blur)(this.textArea) }
  onScroll() { noop() }
  render() {
    return <textarea
      className="string i"
      rows={1}
      wrap="off"
      spellCheck={false}
      onChange={e => { if (this.props.stringEditor.stringEditorSelectedState && this.props.stringEditor.stringEditorSelectedState.writable)
        this.props.runE(() => bindMaybe(cursorFromD(this.props.stringEditor), cursor => mapMaybe(guidFromID(cursor.parent), guid => {if (this.textArea) set(guid, cursor.label, sidFromString(this.textArea.value))})))}}
      value={this.props.stringEditor.string}
      onFocus={e => handleFocusEvent(() => this.props.runE(() => mapMaybe(cursorFromD(this.props.stringEditor), cursor => environment().selection = {cursor})))}
      onBlur={e => handleFocusEvent(() => this.props.runE(() => environment().selection = nothing))}
      onClick={e => e.stopPropagation() }
      onKeyDown={e => this.onKeyDown(e)}
      ref={input => { this.textArea = input }} /> }
  resizeTextArea() { if (this.textArea) {
    const sizeBuffer = 2
    this.textArea.style.width = this.textArea.style.height = "0"
    this.textArea.style.width = `${this.textArea.scrollWidth + sizeBuffer}px`
    this.textArea.style.height = `${this.textArea.scrollHeight + sizeBuffer}px` }}
  componentDidMount() { this.resizeTextArea(); this.focusIfSelected() }
  componentDidUpdate() { this.resizeTextArea(); this.focusIfSelected() } }
