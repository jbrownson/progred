import * as React from 'react'
import { concatMap, intersperse, join } from "../../lib/Array"
import { bindMaybe, mapMaybe, maybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { chooseIDForSelection } from "../editor/chooseIDForSelection"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { cursorFromD } from "../cursor/cursorFromD"
import { Block, D, Descend, GuidEditor, Label, matchD } from "../render/D"
import { _get, environment } from "../Environment"
import { NumberEditorComponent } from "./NumberEditorComponent"
import { PlaceholderComponent } from "./PlaceholderComponent"
import { SelectionState } from "../editor/selectionIfSelected"
import { StringEditorComponent } from "./StringEditorComponent"
import { IdenticonComponent } from "./IdenticonComponent"
import { ID } from "../model/ID"
import { cursorsEqual } from "../cursor/Cursor"
import { blur, focus, handleFocusEvent } from "../editor/ignoreFocusEvents"

const indentWidth = 16

function clickedIDFromD(d: D): Maybe<ID> {
  return d instanceof Label ? d.cursor.label
    : d instanceof Descend ? _get(d.cursor.parent, d.cursor.label)
    : bindMaybe(d.parent, clickedIDFromD) }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !line.children.find(child => !isSingleLine(child)), dText => true, dIdenticon => true, dList => dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), guidEditor => isSingleLine(guidEditor.child), supportsUnderselection => isSingleLine(supportsUnderselection.child), label => isSingleLine(label.child), collapseToggle => true, button => true, placeholder => true, stringEditor => true, numberEditor => true) }

export class DComponent extends React.Component<{d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  children: (DComponent | PlaceholderComponent | StringEditorComponent | NumberEditorComponent | GuidEditorComponent)[]
  onScroll() { this.children.forEach(child => child.onScroll()) }
  render() {
    this.children = []
    let addChild = (child: DComponent | PlaceholderComponent | StringEditorComponent | NumberEditorComponent | GuidEditorComponent | null) => { if (child) this.children.push(child) }
    let chooseID = () => maybe(clickedIDFromD(this.props.d), () => false, chooseIDForSelection)
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
        return dList.collapseToggle && dList.collapseToggle.collapsed
          ? <span>{collapseToggle}<span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dList.opening}</span><span className="collapsedListContents">...</span><span>{dList.closing}</span></span>
          : dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child))
          // TOOD probably something to factor out of these two clauses
          ? <span>{collapseToggle}<span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dList.opening}</span><span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(0)) }}> </span>{
            dList.children.map(child => <DComponent ref={addChild} d={child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />) }
            <span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(dList.children.length)) }}> {dList.closing}</span></span>
          : <span>{collapseToggle}<span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dList.opening}</span><span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(0)) }}> </span>{join(intersperse(
            dList.children.map(child => [<br />, <span style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />, <DComponent ref={addChild} d={child} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} />]),
            i => [<span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(i)) }}>{dList.separator}</span>]))}
            <span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(dList.children.length)) }}> {dList.closing}</span></span> },
      descend => {
        let classNames = ["descend", ...maybeMap([[descend.selectionState === SelectionState.Selected, "selected"], [descend.unmatching, "unmatching"], [descend.selectionState === SelectionState.Hinted, "hinted"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
        return <span className={classNames.join(" ")}><DComponent ref={addChild} d={descend.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /></span> },
      guidEditor => <GuidEditorComponent ref={addChild} guidEditor={guidEditor} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      supportsUnderselection => <DComponent ref={addChild} d={supportsUnderselection.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      label => <span className="edgeLabel"><DComponent ref={addChild} d={label.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /></span>,
      collapseToggle => <span className="collapseToggle" onClick={e => { e.stopPropagation(); this.props.runE(collapseToggle.action) }}>{collapseToggle.collapsed ? "▸" : "▾"}</span>,
      button => <input type="button" value={button.text} onClick={e => { e.stopPropagation(); this.props.runE(button.action) }} />,
      placeholder => <PlaceholderComponent ref={addChild} placeholder={placeholder} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      stringEditor => <StringEditorComponent ref={addChild} stringEditor={stringEditor} runE={this.props.runE} />,
      numberEditor => <NumberEditorComponent ref={addChild} numberEditor={numberEditor} runE={this.props.runE} /> )}}

class GuidEditorComponent extends React.Component<{guidEditor: GuidEditor, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  span: HTMLSpanElement | null
  child: Maybe<DComponent> = nothing
  onScroll() { if (this.child) this.child.onScroll() }
  focusIfSelected() {
    if (this.span) {
      (this.props.guidEditor.selectionState === SelectionState.Selected && this.props.guidEditor.focusWhenSelected ? focus : blur)(this.span) }}
  render() {
    return <span
      className="guidEditor"
      tabIndex={0}
      onMouseDown={e => { if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) e.preventDefault() }}
      onClick={e => { e.stopPropagation(); this.props.runE(() => environment().selection = {cursor: this.props.guidEditor.cursor}) }}
      onFocus={e => handleFocusEvent(() => this.props.runE(() => environment().selection = {cursor: this.props.guidEditor.cursor}))}
      onBlur={e => handleFocusEvent(() => this.props.runE(() => {
        if (environment().selection && cursorsEqual(environment().selection.cursor, this.props.guidEditor.cursor)) environment().selection = nothing }))}
      ref={span => { this.span = span }} >
      <DComponent
        ref={dComponent => { this.child = dComponent || nothing }}
        d={this.props.guidEditor.child}
        depth={this.props.depth}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE} />
    </span> }
  componentDidMount() { this.focusIfSelected() }
  componentDidUpdate() { this.focusIfSelected() } }
