import * as React from 'react'
import { getTextWidth } from "../../lib/getTextWidth"
import { fromMaybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import type { EditorDescend, NumberEditor } from "../render/Projection"
import { nidFromNumber } from "../model/ID"
import { editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { useEditorAttachment } from "./useEditorAttachment"

export function NumberEditorComponent(props: {numberEditor: NumberEditor, editorCommands: EditorCommands, cursor?: Cursor, descend?: EditorDescend, runE: (f: () => void) => void}) {
  const [editedValue, setEditedValue] = React.useState<string | undefined>(undefined)
  const input = React.useRef<HTMLInputElement | null>(null)
  useEditorAttachment(input, props.editorCommands, {cursor: props.cursor, descend: props.descend})
  let commit = (value: string) => {
    let number = +value
    if (!isNaN(number) && props.numberEditor.writable) {
      setEditedValue(undefined)
      props.runE(() => props.editorCommands.commit?.(nidFromNumber(number))) }}
  let onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    let keyDownAction = editorKeyDownAction(props.editorCommands, e)
    if (keyDownAction) { props.runE(keyDownAction); return }
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) {
      stopPropagationForTextInputs(e)
      if (e.key === "Enter") {
        e.preventDefault()
        e.stopPropagation()
        commit(e.currentTarget.value) }}}
  const value = fromMaybe(editedValue, () => `${props.numberEditor.number}`)
  return <input
    className={"number i"}
    type="text"
    style={{width: getTextWidth(value) + "px"}}
    onChange={e => { if (props.numberEditor.writable) setEditedValue(e.currentTarget.value) }}
    onBlur={() => setEditedValue(undefined)}
    value={value}
    onClick={e => e.stopPropagation()}
    onKeyDown={onKeyDown}
    ref={input} />
}
