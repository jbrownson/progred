import * as React from 'react'
import { getTextWidth } from "../lib/getTextWidth"
import { bindMaybe, fromMaybe, mapMaybe, maybe, nothing } from "../lib/Maybe"
import { noop } from "../lib/noop"
import { cursorFromD } from "./cursorFromD"
import { NumberEditor } from "./D"
import { environment, set } from "./Environment"
import { guidFromID, nidFromNumber } from "./ID"
import { blur, focus, handleFocusEvent } from "./ignoreFocusEvents"
import { stopPropagationForTextInputs } from "./stopPropagationForTextInputs"

export class NumberEditorComponent extends React.Component<{numberEditor: NumberEditor, runE: (f: () => void) => void}, {}> {
  input: HTMLInputElement | null
  onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) {
      stopPropagationForTextInputs(e)
      if (e.key === "Enter") {
        e.preventDefault()
        e.stopPropagation()
        this.commit(e.currentTarget.value) }}}
    commit(value: string) {
      let number = +value
      if (!isNaN(number)) this.props.runE(() => bindMaybe(cursorFromD(this.props.numberEditor), cursor => mapMaybe(guidFromID(cursor.parent), guid => set(guid, cursor.label, nidFromNumber(number))))) }
  focusIfSelected() { if (this.input) maybe(this.props.numberEditor.numberEditorSelectedState, () => blur, () => focus)(this.input) }
  onScroll() { noop() }
  render(): JSX.Element {
    const value = maybe(this.props.numberEditor.numberEditorSelectedState, () => "" + this.props.numberEditor.number, ({numberEditorState}) => fromMaybe(numberEditorState.value, () => `${this.props.numberEditor.number}`))
    return <input
      className={"number i"}
      type="text"
      style={{width: getTextWidth(value) + "px"}}
      onChange={e => { if (this.input) { let input = this.input; mapMaybe(this.props.numberEditor.numberEditorSelectedState, numberEditorSelectedState => {
        if (numberEditorSelectedState.writable)
          this.props.runE(() => numberEditorSelectedState.numberEditorState.value = input.value) })}}}
      onFocus={e => handleFocusEvent(() => this.props.runE(() => mapMaybe(cursorFromD(this.props.numberEditor), cursor => environment().selection = {cursor})))}
      onBlur={e => handleFocusEvent(() => this.props.runE(() => environment().selection = nothing))}
      value={value}
      onClick={e => e.stopPropagation()}
      onKeyDown={e => this.onKeyDown(e) }
      ref={input => this.input = input} /> }
  componentDidMount() { this.focusIfSelected() }
  componentDidUpdate() { this.focusIfSelected() } }