import * as React from "react"
import { getTextWidth } from "../../lib/getTextWidth"
import { makeElementVisible } from "../../lib/makeElementVisible"
import { fromMaybe, mapMaybe, maybe, nothing } from "../../lib/Maybe"
import { cursorFromD } from "../cursor/cursorFromD"
import { createD, PlaceholderEditor, PlaceholderEditorSelectedState } from "../render/D"
import { Entry } from "../editor/Entry"
import { environment } from "../Environment"
import { Match } from "../editor/filters"
import { sidFromString } from "../model/ID"
import { focus, handleFocusEvent } from "../editor/ignoreFocusEvents"
import { doTab } from "../editor/keyHandler"
import { appendToListCursor, insertAfterListElemCursor, selectionCursorBindMaybe, setCursorToEmptyList } from "../editor/listCursorActions"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { attachEditorCommands, detachEditorCommands } from "../editor/EditorCommands"

class EntryList extends React.Component<{placeholderEditor: PlaceholderEditor, selectedState: PlaceholderEditorSelectedState, entries: {a: Entry, matches: Match[]}[], runE: (f: () => void) => void, close: () => void}, {}> {
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
      ? this.commitAction()
      : nothing }
  commitAndAdvance(e: React.KeyboardEvent<HTMLInputElement>) {
    mapMaybe(this.commitAction(), action => {
      e.preventDefault()
      e.stopPropagation()
      this.props.runE(() => {
        action()
        let {rootDescend, viewsDescend} = createD()
        doTab(false, rootDescend, viewsDescend) })})}
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
      case "[":
        let value = this.props.selectedState.editorState.value
        if (value === nothing || value === "") {
          this.props.runE(() => selectionCursorBindMaybe(cursor =>
            mapMaybe(setCursorToEmptyList(cursor), cursor => {
              e.preventDefault()
              e.stopPropagation()
              maybe(appendToListCursor(cursor),
                () => environment().selection = {cursor},
                cursor => environment().selection = {cursor} )})))}
        break
      case "Escape":
        e.preventDefault()
        e.stopPropagation()
        this.props.close()
        break
      case ",":
        if (!e.shiftKey || e.ctrlKey || e.altKey || e.metaKey)
          this.props.runE(() => {
            mapMaybe(selectionCursorBindMaybe(cursor => insertAfterListElemCursor(cursor)), cursor => {
              e.preventDefault()
              e.stopPropagation()
              environment().selection = {cursor}
              mapMaybe(this.commitActionIfSomethingToCommit(), () => mapMaybe(this.commitAction(), action => action())) })})
        break
      case "Tab":
        this.commitAndAdvance(e)
        break }}
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
            this.props.runE(action) }}>
            {renderMatches(string, matches)}{maybe(disambiguation, () => nothing, disambiguation => <span className="disambiguation">{disambiguation}</span>)}</li>)}</ul></div> }}

function renderMatches(string: string, matches: Match[]) {
  let {index, strings} = matches.reduce(
    (a, match) => ({index: match.start + match.length, strings: [
      ...a.strings,
      ...a.index < match.start ? [{string: string.slice(a.index, match.start), matching: false}] : [],
      ...match.length > 0 ? [{string: string.slice(match.start, match.start + match.length), matching: true}] : [] ]}),
    {index: 0, strings: new Array<{string: string, matching: boolean}>()} )
  return [...strings, ...index < string.length ? [{string: string.slice(index), matching: false}] : []]
    .map(({string, matching}) => <span className={matching ? "matching" : ""}>{string}</span>) }

export class PlaceholderEditorComponent extends React.Component<{placeholderEditor: PlaceholderEditor, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  entryList: EntryList | null
  input: HTMLInputElement | null
  open(selectedState: PlaceholderEditorSelectedState) {
    selectedState.editorState.completionOpen = true
    this.forceUpdate() }
  close(selectedState: PlaceholderEditorSelectedState) {
    selectedState.editorState.completionOpen = false
    selectedState.editorState.value = ""
    selectedState.editorState.itemSelection = nothing
    this.forceUpdate() }
  updateEntryListAbove() {
    if (this.input && this.entryList && this.entryList.div) {
      let scrollParent = this.props.scrollParent()
      if (scrollParent) {
        const entryListAbove = this.input.getBoundingClientRect().bottom + this.entryList.div.clientHeight > scrollParent.clientTop + scrollParent.clientHeight
        if (entryListAbove !== this.entryList.props.selectedState.editorState.entryListAbove)
          this.entryList.props.selectedState.editorState.entryListAbove = entryListAbove
          this.entryList.forceUpdate() }}}
  focusIfSelected() { if (this.input) focus(this.input) }
  attachEditorCommands() {
    if (this.input) attachEditorCommands(this.input, this.props.placeholderEditor.editorCommands) }
  onScroll() { this.updateEntryListAbove() }
  render() {
    return maybe(this.props.placeholderEditor.selectedState, () =>
      <span className="uneditable" onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.placeholderEditor), cursor => environment().selection = {cursor})) }} >{this.props.placeholderEditor.name}</span>,
    selectedState =>
      <span className="edgefield" style={{position: "relative"}}>
        <input
          ref={input => { if (this.input && this.input !== input) detachEditorCommands(this.input); this.input = input }}
          className="i edgefield"
          style={{width: getTextWidth(selectedState.editorState.value || this.props.placeholderEditor.name) + "px"}}
          type="text"
          placeholder={this.props.placeholderEditor.name}
          value={fromMaybe(selectedState.editorState.value, () => "")}
          onPaste={e => {
            let s = e.clipboardData.getData("text/plain")
            if (s.indexOf("\n") >= 0)
              this.props.runE(() => mapMaybe(this.props.placeholderEditor.editorCommands.commitID, commitID => commitID(sidFromString(s)))) }}
          onBlur={e => handleFocusEvent(() => this.props.runE(() => { e.currentTarget.value = ""; environment().selection = nothing }))}
          onClick={e => e.stopPropagation()}
          onKeyDown={e => {
            if ((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0) return
            if (selectedState.editorState.completionOpen && this.entryList) {
              stopPropagationForTextInputs(e)
              this.entryList.onKeyDown(e) }
            else {
              switch (e.key) {
                case "Enter":
                  e.preventDefault()
                  e.stopPropagation()
                  this.open(selectedState)
                  break
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
                case "Escape":
                  e.preventDefault()
                  e.stopPropagation()
                  this.props.runE(() => environment().selection = nothing)
                  break
                default:
                  stopPropagationForTextInputs(e) }}}}
          onChange={e => { if (this.input) { selectedState.editorState.value = this.input.value; selectedState.editorState.itemSelection = nothing; selectedState.editorState.completionOpen = true; this.forceUpdate() } } } />
      {selectedState.editorState.completionOpen ? <EntryList ref={entryList => { this.entryList = entryList }} placeholderEditor={this.props.placeholderEditor} selectedState={selectedState} entries={selectedState.entries(fromMaybe(selectedState.editorState.value, () => ""))} runE={this.props.runE} close={() => this.close(selectedState)} /> : null}</span> )}
  componentDidMount() { this.focusIfSelected(); this.attachEditorCommands(); this.updateEntryListAbove() }
  componentDidUpdate() { this.focusIfSelected(); this.attachEditorCommands(); this.updateEntryListAbove() }
  componentWillUnmount() { if (this.input) detachEditorCommands(this.input) }}
