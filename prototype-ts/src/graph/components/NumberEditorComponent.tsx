import * as React from 'react'
import { getTextWidth } from "../../lib/getTextWidth"
import { fromMaybe, mapMaybe } from "../../lib/Maybe"
import { noop } from "../../lib/noop"
import { cursorFromD, descendFromD } from "../cursor/cursorFromD"
import { NumberEditor } from "../render/D"
import { nidFromNumber } from "../model/ID"
import { attachEditorCommands, detachEditorCommands, editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"

type NumberEditorComponentState = {value?: string}

export class NumberEditorComponent extends React.Component<{numberEditor: NumberEditor, editorCommands: EditorCommands, runE: (f: () => void) => void}, NumberEditorComponentState> {
  state: NumberEditorComponentState = {}
  input: HTMLInputElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    let keyDownAction = editorKeyDownAction(this.props.editorCommands, e)
    if (keyDownAction) { this.props.runE(keyDownAction); return }
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) {
      stopPropagationForTextInputs(e)
      if (e.key === "Enter") {
        e.preventDefault()
        e.stopPropagation()
        this.commit(e.currentTarget.value) }}}
    commit(value: string) {
      let number = +value
      if (!isNaN(number) && this.props.numberEditor.writable) {
        this.setState({value: undefined})
        this.props.runE(() => mapMaybe(this.props.editorCommands.commit, commit => commit(nidFromNumber(number)))) }}
  attachEditorCommands() {
    if (this.input) {
      attachEditorCommands(this.input, this.props.editorCommands)
      mapMaybe(cursorFromD(this.props.numberEditor), cursor => attachEditorFocus(this.input!, {cursor, descend: descendFromD(this.props.numberEditor)})) }}
  onScroll() { noop() }
  render() {
    const value = fromMaybe(this.state.value, () => `${this.props.numberEditor.number}`)
    return <input
      className={"number i"}
      type="text"
      style={{width: getTextWidth(value) + "px"}}
      onChange={e => { if (this.input && this.props.numberEditor.writable) this.setState({value: this.input.value}) }}
      onBlur={() => this.setState({value: undefined})}
      value={value}
      onClick={e => e.stopPropagation()}
      onKeyDown={e => this.onKeyDown(e) }
      ref={input => { this.input = input }} /> }
  componentDidMount() { this.attachEditorCommands() }
  componentDidUpdate() { this.attachEditorCommands() }
  componentWillUnmount() { if (this.input) { detachEditorCommands(this.input); detachEditorFocus(this.input) } } }
