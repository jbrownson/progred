import * as React from "react"
import { getTextWidth } from "../../lib/getTextWidth"
import { makeElementVisible } from "../../lib/makeElementVisible"
import { fromMaybe, mapMaybe, maybe, nothing } from "../../lib/Maybe"
import type { EditorDescend } from "../render/DContext"
import type { PlaceholderEditorActiveState } from "../render/DEditors"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import { focus } from "../editor/domFocus"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { EditorCommands } from "../editor/EditorCommands"
import { registerScrollListener } from "../editor/ScrollListeners"
import { useEditorAttachment } from "./useEditorAttachment"

type EntryListProps = {activeState: PlaceholderEditorActiveState, entries: {a: Entry, matches: Match[]}[], close: () => void, commit: (action: () => void, e: React.SyntheticEvent) => void, keyDown?: (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => void}
type EntryListHandle = {readonly div: HTMLElement | null, onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void, forceUpdate: () => void}

const EntryList = React.forwardRef<EntryListHandle, EntryListProps>(function EntryList(props, ref) {
  const div = React.useRef<HTMLDivElement | null>(null)
  const lis = React.useRef(new Map<number, HTMLElement>())
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const li = (index: number): HTMLElement => lis.current.get(index) as HTMLElement
  const up = (itemSelection: number) => {
    let newItemSelection = Math.max(0, itemSelection - 1)
    props.activeState.editorState.itemSelection = newItemSelection
    let selectedLI = li(newItemSelection)
    makeElementVisible(selectedLI, selectedLI.parentNode as HTMLElement)
    forceUpdate() }
  const down = () => {
    if (props.entries.length === 0) return
    let newItemSelection = maybe(props.activeState.editorState.itemSelection, () => 0, selection => Math.min(props.entries.length - 1, selection + 1))
    props.activeState.editorState.itemSelection = newItemSelection
    let selectedLI = li(newItemSelection)
    makeElementVisible(selectedLI, selectedLI.parentNode as HTMLElement)
    forceUpdate() }
  const commitAction = () => maybe(props.activeState.editorState.itemSelection,
    () => mapMaybe(props.entries[0], first => first.a.action),
    i => mapMaybe(props.entries[i], entry => entry.a.action) )
  const commitActionIfSomethingToCommit = () => {
    let value = props.activeState.editorState.value
    return (value !== nothing && value !== "") || props.activeState.editorState.itemSelection !== nothing
      ? mapMaybe(commitAction(), action => action())
      : nothing }
  const commitAndAdvance = (e: React.KeyboardEvent<HTMLInputElement>) => {
    mapMaybe(commitAction(), action => {
      props.commit(action, e) })}
  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case "ArrowUp":
        e.preventDefault()
        e.stopPropagation()
        maybe(props.activeState.editorState.itemSelection, () => { if (props.entries.length > 0) up(0) }, itemSelection => { if (itemSelection > 0) up(itemSelection) })
        break
      case "ArrowDown": e.preventDefault(); e.stopPropagation(); down(); break
      case "Enter": commitAndAdvance(e); break
      case "Escape":
        e.preventDefault()
        e.stopPropagation()
        props.close()
        break
      case "Tab":
        commitAndAdvance(e)
        break
      default:
        maybe(props.keyDown, () => {}, keyDown => keyDown(e, commitActionIfSomethingToCommit)) }}
  React.useImperativeHandle(ref, () => ({
    get div() { return div.current },
    onKeyDown,
    forceUpdate }))
  return <div ref={div} className="entrylist" style={props.activeState.editorState.entryListAbove ? {bottom: "100%"} : {}}><ul>{
    props.entries.map(({a: {string, disambiguation, matching, action, external}, matches}, i) =>
      <li
        key={i}
        ref={li => { if (li) lis.current.set(i, li); else lis.current.delete(i) }}
        className={[
          ...i === props.activeState.editorState.itemSelection ? ["selected"] : [],
          matching ? "matching" : "unmatching",
          ...external ? ["external"] : [] ]
          .join(" ") }
        onMouseMove={() => { if (i !== props.activeState.editorState.itemSelection) {props.activeState.editorState.itemSelection = i; forceUpdate()} }}
        onClick={e => e.stopPropagation()}
        onMouseDown={e => {
          e.preventDefault()
          props.commit(action, e) }}>
          {renderMatches(string, matches)}{maybe(disambiguation, () => nothing, disambiguation => <span className="disambiguation">{disambiguation}</span>)}</li>)}</ul></div>
})

function renderMatches(string: string, matches: Match[]) {
  let {index, strings} = matches.reduce(
    (a, match) => ({index: match.start + match.length, strings: [
      ...a.strings,
      ...a.index < match.start ? [{string: string.slice(a.index, match.start), matching: false}] : [],
      ...match.length > 0 ? [{string: string.slice(match.start, match.start + match.length), matching: true}] : [] ]}),
    {index: 0, strings: new Array<{string: string, matching: boolean}>()} )
  return [...strings, ...index < string.length ? [{string: string.slice(index), matching: false}] : []]
    .map(({string, matching}, index) => <span key={index} className={matching ? "matching" : ""}>{string}</span>) }

export function PlaceholderInputComponent(props: {activeState: PlaceholderEditorActiveState, placeholder: string, editorCommands: EditorCommands, edge?: Edge, descend?: EditorDescend, tabStop?: boolean, runE: (f: () => void) => void, closeCompletion: () => void, cancel: () => void, blur: () => void, commit: (action: () => void, e: React.SyntheticEvent) => void, keyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void, entryListKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => void}) {
  const entryList = React.useRef<EntryListHandle | null>(null)
  const input = React.useRef<HTMLInputElement | null>(null)
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  useEditorAttachment(input, props.editorCommands, {edge: props.edge, descend: props.descend, tabStop: props.tabStop})
  const updateEntryListAbove = () => {
    if (input.current && entryList.current && entryList.current.div) {
      const entryListAbove = input.current.getBoundingClientRect().bottom + entryList.current.div.clientHeight > window.innerHeight
      if (entryListAbove !== props.activeState.editorState.entryListAbove) {
        props.activeState.editorState.entryListAbove = entryListAbove
        entryList.current.forceUpdate() }}}
  React.useEffect(() => registerScrollListener(updateEntryListAbove))
  React.useLayoutEffect(() => { if (input.current) focus(input.current) }, [])
  React.useLayoutEffect(() => updateEntryListAbove())
  return <span className="edgefield" style={{position: "relative"}}>
    <input
      ref={input}
      className="i edgefield"
      style={{width: getTextWidth(props.activeState.editorState.value || props.placeholder) + "px"}}
      type="text"
      placeholder={props.placeholder}
      value={fromMaybe(props.activeState.editorState.value, () => "")}
      onPaste={e => {
        let s = e.clipboardData.getData("text/plain")
        if (s.indexOf("\n") >= 0)
          props.runE(() => props.editorCommands.commit?.(sidFromString(s))) }}
      onBlur={() => props.blur()}
      onClick={e => e.stopPropagation()}
      onKeyDown={e => {
        if ((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0) return
        if (props.activeState.editorState.completionOpen && entryList.current) {
          stopPropagationForTextInputs(e)
          entryList.current.onKeyDown(e) }
        else {
          switch (e.key) {
            case "Enter":
              e.preventDefault()
              e.stopPropagation()
              props.activeState.editorState.completionOpen = true
              forceUpdate()
              break
            case "Escape":
              e.preventDefault()
              e.stopPropagation()
              props.cancel()
              break
            default:
              maybe(props.keyDown, () => stopPropagationForTextInputs(e), keyDown => keyDown(e)) }}}}
      onChange={e => {
        props.activeState.editorState.value = e.currentTarget.value
        props.activeState.editorState.itemSelection = nothing
        props.activeState.editorState.completionOpen = true
        forceUpdate() }} />
    {props.activeState.editorState.completionOpen ? <EntryList ref={entryList} activeState={props.activeState} entries={props.activeState.entries(fromMaybe(props.activeState.editorState.value, () => ""))} close={props.closeCompletion} commit={props.commit} keyDown={props.entryListKeyDown} /> : null}</span>
}
