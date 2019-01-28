import * as React from "react"
import { getTextWidth } from "../lib/getTextWidth"
import { makeElementVisible } from "../lib/makeElementVisible"
import { bindMaybe, fromMaybe, mapMaybe, maybe, nothing } from "../lib/Maybe"
import { cursorFromD } from "./cursorFromD"
import { createD, Placeholder, PlaceholderSelectedState } from "./D"
import { Entry } from "./Entry"
import { environment, set } from "./Environment"
import { Match } from "./filters"
import { guidFromID, sidFromString } from "./ID"
import { focus, handleFocusEvent } from "./ignoreFocusEvents"
import { doTab } from "./keyHandler"
import { appendToListCursor, insertAfterListElemCursor, selectionCursorBindMaybe, setCursorToEmptyList } from "./listCursorActions"
import { stopPropagationForTextInputs } from "./stopPropagationForTextInputs"

class EntryList extends React.Component<{placeholder: Placeholder, selectedState: PlaceholderSelectedState, entries: {a: Entry, matches: Match[]}[], runE: (f: () => void) => void}, {}> {
  div: HTMLElement | null
  liRef(index: number) { return `entry${index}` }
  li(index: number): HTMLElement { return this.refs[this.liRef(index)] as HTMLElement }
  up(itemSelection: number) {
    let newItemSelection = Math.max(0, itemSelection - 1)
    this.props.selectedState.placeholderState.itemSelection = newItemSelection
    let li = this.li(newItemSelection)
    makeElementVisible(li, (li.parentNode as HTMLElement).parentNode as HTMLElement)
    this.forceUpdate() }
  down() {
    let newItemSelection = maybe(this.props.selectedState.placeholderState.itemSelection, () => 0, selection => Math.min(this.props.entries.length - 1, selection + 1))
    this.props.selectedState.placeholderState.itemSelection = newItemSelection
    let li = this.li(newItemSelection)
    makeElementVisible(li, (li.parentNode as HTMLElement).parentNode as HTMLElement)
    this.forceUpdate() }
  commitActionIfSomethingToCommit() {
    let value = this.props.selectedState.placeholderState.value
    return (value !== nothing && value !== "") || this.props.selectedState.placeholderState.itemSelection !== nothing
      ? this.commitAction()
      : nothing }
  tab(e: React.KeyboardEvent<HTMLInputElement>) {
    mapMaybe(this.commitActionIfSomethingToCommit(), () => {
      mapMaybe(this.commitAction(), action => {
        e.preventDefault()
        e.stopPropagation()
        this.props.runE(() => { action(); let {rootDescend, viewsDescend} = createD(); doTab(e.shiftKey, rootDescend, viewsDescend) }) })})}
  deselect() {
    mapMaybe(this.props.selectedState.placeholderState.itemSelection, is => {
      this.props.selectedState.placeholderState.itemSelection = nothing
      mapMaybe(this.props.entries[0], first => {
        let li = this.li(0)
        makeElementVisible(li, (li.parentNode as HTMLElement).parentNode as HTMLElement)})
      this.forceUpdate() }) }
  commit() { mapMaybe(this.commitAction(), commitAction => this.props.runE(commitAction)) }
  commitAction() {
    return maybe(this.props.selectedState.placeholderState.itemSelection,
      () => mapMaybe(this.props.entries[0], first => first.a.action),
      i => mapMaybe(this.props.entries[i], entry => entry.a.action) )}
  onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    switch (e.key) {
      case "ArrowUp": bindMaybe(this.props.selectedState.placeholderState.itemSelection, itemSelection => { if (itemSelection > 0) { e.preventDefault(); e.stopPropagation(); this.up(itemSelection) } }); break
      case "ArrowDown": e.preventDefault(); e.stopPropagation(); this.down(); break
      case "Enter": e.preventDefault(); e.stopPropagation(); this.commit(); break
      case "[":
        let value = this.props.selectedState.placeholderState.value
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
        mapMaybe(this.props.selectedState.placeholderState.itemSelection, () => {
          e.preventDefault()
          e.stopPropagation()
          this.deselect() })
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
        this.tab(e)
        break }}
  render(): JSX.Element {
    return <div ref={div => this.div = div} className="entrylist" style={this.props.selectedState.placeholderState.entryListAbove ? {bottom: 17} : {}}><ul>{
      this.props.entries.map(({a: {string, disambiguation, matching, action, external}, matches}, i) =>
        <li
          key={i}
          ref={this.liRef(i)}
          className={[
            ...i === this.props.selectedState.placeholderState.itemSelection ? ["selected"] : [],
            matching ? "matching" : "unmatching",
            ...external ? ["external"] : [] ]
            .join(" ") }
          onMouseMove={() => { if (i !== this.props.selectedState.placeholderState.itemSelection) {this.props.selectedState.placeholderState.itemSelection = i; this.forceUpdate()} }}
          onClick={e => e.stopPropagation()}
          onMouseDown={() => this.props.runE(action)}>
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

export class PlaceholderComponent extends React.Component<{placeholder: Placeholder, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  entryList: EntryList | null
  input: HTMLInputElement | null
  updateEntryListAbove() {
    if (this.input && this.entryList && this.entryList.div) {
      let scrollParent = this.props.scrollParent()
      if (scrollParent) {
        const entryListAbove = this.input.getBoundingClientRect().bottom + this.entryList.div.clientHeight > scrollParent.clientTop + scrollParent.clientHeight
        if (entryListAbove !== this.entryList.props.selectedState.placeholderState.entryListAbove)
          this.entryList.props.selectedState.placeholderState.entryListAbove = entryListAbove
          this.entryList.forceUpdate() }}}
  focusIfSelected() { if (this.input) focus(this.input) }
  onScroll() { this.updateEntryListAbove() }
  render(): JSX.Element {
    return maybe(this.props.placeholder.selectedState, () =>
      <span className="uneditable" onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.placeholder), cursor => environment().selection = {cursor})) }} >{this.props.placeholder.name}</span>,
    selectedState =>
      <span className="edgefield" style={{position: "relative"}}>
        <input
          ref={input => this.input = input}
          className="i edgefield"
          style={{width: getTextWidth(selectedState.placeholderState.value || this.props.placeholder.name) + "px"}}
          type="text"
          placeholder={this.props.placeholder.name}
          value={fromMaybe(selectedState.placeholderState.value, () => "")}
          onPaste={e => {
            let s = e.clipboardData.getData("text/plain")
            if (s.indexOf("\n") >= 0)
              this.props.runE(() => bindMaybe(cursorFromD(this.props.placeholder), cursor => mapMaybe(guidFromID(cursor.parent), guid => {set(guid, cursor.label, sidFromString(s))}))) }}
          onBlur={e => handleFocusEvent(() => this.props.runE(() => { e.currentTarget.value = ""; environment().selection = nothing }))}
          onClick={e => e.stopPropagation()}
          onKeyDown={e => {
            if (this.entryList && !((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) {
              stopPropagationForTextInputs(e)
              this.entryList.onKeyDown(e) }}}
          onChange={e => { if (this.input) { selectedState.placeholderState.value = this.input.value; selectedState.placeholderState.itemSelection = nothing; this.forceUpdate() } } } />
      <EntryList ref={entryList => this.entryList = entryList} placeholder={this.props.placeholder} selectedState={selectedState} entries={selectedState.entries(fromMaybe(selectedState.placeholderState.value, () => ""))} runE={this.props.runE} /></span> )}
  componentDidMount() { this.focusIfSelected(); this.updateEntryListAbove() }
  componentDidUpdate() { this.focusIfSelected(); this.updateEntryListAbove() }}