import * as React from "react"
import { getTextWidth } from "../../lib/getTextWidth"
import { makeElementVisible } from "../../lib/makeElementVisible"
import { fromMaybe, mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { PlaceholderEditorSelectedState } from "../render/D"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { sidFromString } from "../model/ID"
import { focus, handleFocusEvent } from "../editor/ignoreFocusEvents"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { attachEditorCommands, detachEditorCommands, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"

class EntryList extends React.Component<{selectedState: PlaceholderEditorSelectedState, entries: {a: Entry, matches: Match[]}[], close: () => void, commit: (action: () => void, e: React.SyntheticEvent) => void, keyDown?: (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => void}, {}> {
  div: HTMLElement | null
  lis = new Map<number, HTMLElement>()
  li(index: number): HTMLElement { return this.lis.get(index) as HTMLElement }
  up(itemSelection: number) {
    let newItemSelection = Math.max(0, itemSelection - 1)
    this.props.selectedState.editorState.itemSelection = newItemSelection
    let li = this.li(newItemSelection)
    makeElementVisible(li, li.parentNode as HTMLElement)
    this.forceUpdate() }
  down() {
    let newItemSelection = maybe(this.props.selectedState.editorState.itemSelection, () => 0, selection => Math.min(this.props.entries.length - 1, selection + 1))
    this.props.selectedState.editorState.itemSelection = newItemSelection
    let li = this.li(newItemSelection)
    makeElementVisible(li, li.parentNode as HTMLElement)
    this.forceUpdate() }
  commitActionIfSomethingToCommit() {
    let value = this.props.selectedState.editorState.value
    return (value !== nothing && value !== "") || this.props.selectedState.editorState.itemSelection !== nothing
      ? mapMaybe(this.commitAction(), action => action())
      : nothing }
  commitAndAdvance(e: React.KeyboardEvent<HTMLInputElement>) {
    mapMaybe(this.commitAction(), action => {
      this.props.commit(action, e) })}
  commitAction() {
    return maybe(this.props.selectedState.editorState.itemSelection,
      () => mapMaybe(this.props.entries[0], first => first.a.action),
      i => mapMaybe(this.props.entries[i], entry => entry.a.action) )}
  onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    switch (e.key) {
      case "ArrowUp":
        e.preventDefault()
        e.stopPropagation()
        maybe(this.props.selectedState.editorState.itemSelection, () => { if (this.props.entries.length > 0) this.up(0) }, itemSelection => { if (itemSelection > 0) this.up(itemSelection) })
        break
      case "ArrowDown": e.preventDefault(); e.stopPropagation(); this.down(); break
      case "Enter": this.commitAndAdvance(e); break
      case "Escape":
        e.preventDefault()
        e.stopPropagation()
        this.props.close()
        break
      case "Tab":
        this.commitAndAdvance(e)
        break
      default:
        maybe(this.props.keyDown, () => {}, keyDown => keyDown(e, () => this.commitActionIfSomethingToCommit())) }}
  render() {
    return <div ref={div => { this.div = div }} className="entrylist" style={this.props.selectedState.editorState.entryListAbove ? {bottom: "100%"} : {}}><ul>{
      this.props.entries.map(({a: {string, disambiguation, matching, action, external}, matches}, i) =>
        <li
          key={i}
          ref={li => { if (li) this.lis.set(i, li); else this.lis.delete(i) }}
          className={[
            ...i === this.props.selectedState.editorState.itemSelection ? ["selected"] : [],
            matching ? "matching" : "unmatching",
            ...external ? ["external"] : [] ]
            .join(" ") }
          onMouseMove={() => { if (i !== this.props.selectedState.editorState.itemSelection) {this.props.selectedState.editorState.itemSelection = i; this.forceUpdate()} }}
          onClick={e => e.stopPropagation()}
          onMouseDown={e => {
            e.preventDefault()
            this.props.commit(action, e) }}>
            {renderMatches(string, matches)}{maybe(disambiguation, () => nothing, disambiguation => <span className="disambiguation">{disambiguation}</span>)}</li>)}</ul></div> }}

function renderMatches(string: string, matches: Match[]) {
  let {index, strings} = matches.reduce(
    (a, match) => ({index: match.start + match.length, strings: [
      ...a.strings,
      ...a.index < match.start ? [{string: string.slice(a.index, match.start), matching: false}] : [],
      ...match.length > 0 ? [{string: string.slice(match.start, match.start + match.length), matching: true}] : [] ]}),
    {index: 0, strings: new Array<{string: string, matching: boolean}>()} )
  return [...strings, ...index < string.length ? [{string: string.slice(index), matching: false}] : []]
    .map(({string, matching}, index) => <span key={index} className={matching ? "matching" : ""}>{string}</span>) }

export class PlaceholderInputComponent extends React.Component<{selectedState: PlaceholderEditorSelectedState, placeholder: string, editorCommands: EditorCommands, cursor?: Cursor, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, closeCompletion: () => void, cancel: () => void, blur: (e: React.FocusEvent<HTMLInputElement>) => void, commit: (action: () => void, e: React.SyntheticEvent) => void, keyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void, entryListKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>, commitActionIfSomethingToCommit: () => void) => void}, {}> {
  entryList: EntryList | null
  input: HTMLInputElement | null
  open() {
    this.props.selectedState.editorState.completionOpen = true
    this.forceUpdate() }
  onScroll() { this.updateEntryListAbove() }
  focusIfSelected() { if (this.input) focus(this.input) }
  attachEditorCommands() {
    if (this.input) {
      attachEditorCommands(this.input, this.props.editorCommands)
      maybe(this.props.cursor, () => detachEditorFocus(this.input!), cursor => attachEditorFocus(this.input!, cursor)) }}
  updateEntryListAbove() {
    if (this.input && this.entryList && this.entryList.div) {
      let scrollParent = this.props.scrollParent()
      if (scrollParent) {
        const entryListAbove = this.input.getBoundingClientRect().bottom + this.entryList.div.clientHeight > scrollParent.clientTop + scrollParent.clientHeight
        if (entryListAbove !== this.entryList.props.selectedState.editorState.entryListAbove)
          this.entryList.props.selectedState.editorState.entryListAbove = entryListAbove
          this.entryList.forceUpdate() }}}
  render() {
    return <span className="edgefield" style={{position: "relative"}}>
      <input
        ref={input => { if (this.input && this.input !== input) { detachEditorCommands(this.input); detachEditorFocus(this.input) } this.input = input }}
        className="i edgefield"
        style={{width: getTextWidth(this.props.selectedState.editorState.value || this.props.placeholder) + "px"}}
        type="text"
        placeholder={this.props.placeholder}
        value={fromMaybe(this.props.selectedState.editorState.value, () => "")}
        onPaste={e => {
          let s = e.clipboardData.getData("text/plain")
          if (s.indexOf("\n") >= 0)
            this.props.runE(() => mapMaybe(this.props.editorCommands.commit, commit => commit(sidFromString(s)))) }}
        onBlur={e => handleFocusEvent(() => this.props.blur(e))}
        onClick={e => e.stopPropagation()}
        onKeyDown={e => {
          if ((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0) return
          if (this.props.selectedState.editorState.completionOpen && this.entryList) {
            stopPropagationForTextInputs(e)
            this.entryList.onKeyDown(e) }
          else {
            switch (e.key) {
              case "Enter":
                e.preventDefault()
                e.stopPropagation()
                this.open()
                break
              case "Escape":
                e.preventDefault()
                e.stopPropagation()
                this.props.cancel()
                break
              default:
                maybe(this.props.keyDown, () => stopPropagationForTextInputs(e), keyDown => keyDown(e)) }}}}
        onChange={e => { if (this.input) { this.props.selectedState.editorState.value = this.input.value; this.props.selectedState.editorState.itemSelection = nothing; this.props.selectedState.editorState.completionOpen = true; this.forceUpdate() } }} />
      {this.props.selectedState.editorState.completionOpen ? <EntryList ref={entryList => { this.entryList = entryList }} selectedState={this.props.selectedState} entries={this.props.selectedState.entries(fromMaybe(this.props.selectedState.editorState.value, () => ""))} close={this.props.closeCompletion} commit={this.props.commit} keyDown={this.props.entryListKeyDown} /> : null}</span> }
  componentDidMount() { this.focusIfSelected(); this.attachEditorCommands(); this.updateEntryListAbove() }
  componentDidUpdate() { this.focusIfSelected(); this.attachEditorCommands(); this.updateEntryListAbove() }
  componentWillUnmount() { if (this.input) { detachEditorCommands(this.input); detachEditorFocus(this.input) } } }
