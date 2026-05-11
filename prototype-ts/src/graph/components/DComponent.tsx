import * as React from 'react'
import { concatMap, intersperse, join } from "../../lib/Array"
import { bindMaybe, mapMaybe, maybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { cursorFromD } from "../cursor/cursorFromD"
import { Block, D, Descend, GuidEditor, Label, matchD } from "../render/D"
import { _get, environment } from "../Environment"
import { NumberEditorComponent } from "./NumberEditorComponent"
import { PlaceholderEditorComponent } from "./PlaceholderEditorComponent"
import { ListInsertionEditorComponent } from "./ListInsertionEditorComponent"
import { SelectionState } from "../editor/selectionIfSelected"
import { StringEditorComponent } from "./StringEditorComponent"
import { IdenticonComponent } from "./IdenticonComponent"
import { ID } from "../model/ID"
import { cursorsEqual } from "../cursor/Cursor"
import { attachEditorCommands, commitIDToActiveElement, detachEditorCommands } from "../editor/EditorCommands"
import { blur, focus, handleFocusEvent } from "../editor/ignoreFocusEvents"

const indentWidth = 16

type DComponentChild = DComponent | PlaceholderEditorComponent | ListInsertionEditorComponent | StringEditorComponent | NumberEditorComponent | GuidEditorComponent
type DComponentState = {activeListInsertion?: number}

function clickedIDFromD(d: D): Maybe<ID> {
  return d instanceof Label ? d.cursor.label
    : d instanceof Descend ? _get(d.cursor.parent, d.cursor.label)
    : bindMaybe(d.parent, clickedIDFromD) }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !line.children.find(child => !isSingleLine(child)), dText => true, dIdenticon => true, dList => dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), guidEditor => isSingleLine(guidEditor.child), supportsUnderselection => isSingleLine(supportsUnderselection.child), label => isSingleLine(label.child), collapseToggle => true, button => true, placeholder => true, stringEditor => true, numberEditor => true) }

export class DComponent extends React.Component<{d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, DComponentState> {
  state: DComponentState = {}
  children: DComponentChild[]
  onScroll() { this.children.forEach(child => child.onScroll()) }
  render() {
    this.children = []
    let addChild = (child: DComponentChild | null) => { if (child) this.children.push(child) }
    let chooseID = () => maybe(clickedIDFromD(this.props.d), () => false, commitIDToActiveElement)
    let keepFocusForChooseID = (e: React.MouseEvent) => {
      if (chooseIDModifier(e)) {
        // Prevent the pending placeholder input from blurring before the click chooses an ID.
        e.stopPropagation()
        e.preventDefault() }}
    let selectOrChooseID = (e: React.MouseEvent) => {
      e.stopPropagation()
      if (chooseIDModifier(e)) {
        e.preventDefault()
        this.props.runE(chooseID)
        return }
      this.props.runE(() => {
        mapMaybe(cursorFromD(this.props.d), cursor => environment().selection = ({cursor})) }) }
    return matchD(this.props.d,
      block => <span>{concatMap(block.children, (d, index) => d instanceof Block
        ? [<DComponent key={`block${index}`} ref={addChild} d={d} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} />]
        : [
          <br key={`br${index}`} />,
          <span key={"span" + index} style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />,
          <DComponent key={`d${index}`} ref={addChild} d={d} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} />])}</span>,
      line => <span>{line.children.map((d, index) => <DComponent ref={addChild} key={index} d={d} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />)}</span>,
      dText => <span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dText.string}</span>,
      dIdenticon => <span className="identicon" onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}><IdenticonComponent guid={dIdenticon.guid} size={dIdenticon.size} /></span>,
      dList => {
        let collapseToggle = dList.collapseToggle ? <DComponent ref={addChild} d={dList.collapseToggle} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /> : null
        let opening = <span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dList.opening}</span>
        let closing = <span>{dList.closing}</span>
        let singleLine = dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child))
        let activeListInsertion = this.state.activeListInsertion !== undefined && dList.insertionPoints[this.state.activeListInsertion] ? this.state.activeListInsertion : undefined
        let setActiveListInsertion = (i: number, active: boolean) => this.setState(({activeListInsertion}) => ({activeListInsertion: active ? i : activeListInsertion === i ? undefined : activeListInsertion}))
        let insertionPoint = (i: number, label: string) => dList.insertionPoints[i]
          ? <ListInsertionEditorComponent key={`insertion${i}`} ref={addChild} insertionPoint={dList.insertionPoints[i]} label={label} active={activeListInsertion === i} setActive={active => setActiveListInsertion(i, active)} scrollParent={this.props.scrollParent} runE={this.props.runE} />
          : <span key={`insertion${i}`}>{label}</span>
        let child = (d: D, i: number, depth: number) => <DComponent key={`child${i}`} ref={addChild} d={d} depth={depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />
        let activeItems = (depth: number) => {
          let items: React.ReactNode[] = []
          for (let i = 0; i <= dList.children.length; i++) {
            if (activeListInsertion === i) items.push(insertionPoint(i, ""))
            if (i < dList.children.length) items.push(child(dList.children[i], i, depth)) }
          return items }
        let content = dList.collapseToggle && dList.collapseToggle.collapsed
          ? [<span key="collapsed" className="collapsedListContents">...</span>]
          : activeListInsertion !== undefined && singleLine
          ? [<span key="leading"> </span>, ...join(intersperse(activeItems(this.props.depth).map(item => [item]), i => [<span key={`separator${i}`}>{dList.separator} </span>])), <span key="trailing"> </span>]
          : activeListInsertion !== undefined
          ? join(activeItems(this.props.depth + 1).map((item, i, items) => [
              <br key={`br${i}`} />,
              <span key={`indent${i}`} style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />,
              item,
              i + 1 < items.length ? <span key={`separator${i}`}>{dList.separator}</span> : null]))
          : singleLine
          ? [insertionPoint(0, " "), ...concatMap(dList.children, (d, i) => [
            child(d, i, this.props.depth),
            insertionPoint(i + 1, " ")])]
          : [insertionPoint(0, " "), ...join(intersperse(
            dList.children.map((d, i) => [
              <br key={`br${i}`} />,
              <span key={`indent${i}`} style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />,
              child(d, i, this.props.depth + 1),
              i === dList.children.length - 1 ? insertionPoint(dList.children.length, " ") : null]),
            i => [insertionPoint(i, dList.separator)]))]
        return <span>{collapseToggle}{opening}{content}{closing}</span> },
      descend => {
        let classNames = ["descend", ...maybeMap([[descend.unmatching, "unmatching"], [descend.selectionState === SelectionState.Hinted, "hinted"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
        return <span className={classNames.join(" ")}><DComponent ref={addChild} d={descend.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /></span> },
      guidEditor => <GuidEditorComponent ref={addChild} guidEditor={guidEditor} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      supportsUnderselection => <DComponent ref={addChild} d={supportsUnderselection.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      label => <span className="edgeLabel"><DComponent ref={addChild} d={label.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /></span>,
      collapseToggle => <span className="collapseToggle" onClick={e => { e.stopPropagation(); this.props.runE(collapseToggle.action) }}>{collapseToggle.collapsed ? "▸" : "▾"}</span>,
      button => <input type="button" value={button.text} onClick={e => { e.stopPropagation(); this.props.runE(button.action) }} />,
      placeholderEditor => <PlaceholderEditorComponent ref={addChild} placeholderEditor={placeholderEditor} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      stringEditor => <StringEditorComponent ref={addChild} stringEditor={stringEditor} runE={this.props.runE} />,
      numberEditor => <NumberEditorComponent ref={addChild} numberEditor={numberEditor} runE={this.props.runE} /> )}}

class GuidEditorComponent extends React.Component<{guidEditor: GuidEditor, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  span: HTMLSpanElement | null
  child: Maybe<DComponent> = nothing
  onScroll() { if (this.child) this.child.onScroll() }
  focusIfSelected() {
    if (this.span) {
      (this.props.guidEditor.selectionState === SelectionState.Selected && this.props.guidEditor.focusWhenSelected ? focus : blur)(this.span) }}
  attachEditorCommands() {
    if (this.span) attachEditorCommands(this.span, this.props.guidEditor.editorCommands) }
  render() {
    return <span
      className="guidEditor"
      tabIndex={0}
      onMouseDown={e => { if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) e.preventDefault() }}
      onClick={e => { e.stopPropagation(); this.props.runE(() => environment().selection = {cursor: this.props.guidEditor.cursor}) }}
      onFocus={e => { if (e.target === e.currentTarget) handleFocusEvent(() => this.props.runE(() => environment().selection = {cursor: this.props.guidEditor.cursor})) }}
      onBlur={e => { if (e.target === e.currentTarget) handleFocusEvent(() => this.props.runE(() => {
        if (environment().selection && cursorsEqual(environment().selection.cursor, this.props.guidEditor.cursor)) environment().selection = nothing })) }}
      ref={span => { this.span = span }} >
      <DComponent
        ref={dComponent => { this.child = dComponent || nothing }}
        d={this.props.guidEditor.child}
        depth={this.props.depth}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE} />
    </span> }
  componentDidMount() { this.focusIfSelected(); this.attachEditorCommands() }
  componentDidUpdate() { this.focusIfSelected(); this.attachEditorCommands() }
  componentWillUnmount() { if (this.span) detachEditorCommands(this.span) } }
