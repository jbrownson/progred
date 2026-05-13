import * as React from 'react'
import { concatMap, intersperse, join } from "../../lib/Array"
import { bindMaybe, mapMaybe, maybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { cursorFromD, descendFromD } from "../cursor/cursorFromD"
import { Block, D, Descend, GuidEditor, Label, matchD, PlaceholderEditorActiveState, PlaceholderEditorState, SupportsUnderselection } from "../render/D"
import { _get } from "../Environment"
import { NumberEditorComponent } from "./NumberEditorComponent"
import { PlaceholderEditorComponent } from "./PlaceholderEditorComponent"
import { PlaceholderInputComponent } from "./PlaceholderInputComponent"
import { ListInsertionEditorComponent } from "./ListInsertionEditorComponent"
import { StringEditorComponent } from "./StringEditorComponent"
import { IdenticonComponent } from "./IdenticonComponent"
import { ID } from "../model/ID"
import { attachEditorCommands, commitIDToActiveElement, detachEditorCommands, editorKeyDownAction } from "../editor/EditorCommands"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { attachEditorDescend, attachEditorFocus, detachEditorFocus, focusEditorForCursor } from "../editor/EditorFocus"
import { focus } from "../editor/ignoreFocusEvents"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { _childCursor } from "../cursor/childCursor"
import { edgeContextFromEdge } from "../editor/edgeContextFromCursor"
import { typeFromEdge } from "../typeFromEdge"
import { renderField } from "../render/defaultRender"

const indentWidth = 16

type DComponentChild = DComponent | PlaceholderEditorComponent | ListInsertionEditorComponent | StringEditorComponent | NumberEditorComponent | GuidEditorComponent | SupportsUnderselectionComponent
type DComponentState = {activeListInsertion?: number}
type DComponentProps = {d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, edgeContext?: EdgeContext, editorCommands?: EditorCommands}

function mergeEditorCommands(parentCommands: Maybe<EditorCommands>, childCommands: EditorCommands): EditorCommands {
  return {
    ...parentCommands,
    ...childCommands }}

function activeEditorCommands(edgeContext: Maybe<EdgeContext>, inheritedCommands: Maybe<EditorCommands>, editorCommands: EditorCommands): EditorCommands {
  return {
    ...inheritedCommands,
    ...editorCommands,
    commit: edgeContext?.commit || editorCommands.commit || inheritedCommands?.commit }}

function clickedIDFromD(d: D): Maybe<ID> {
  return d instanceof Label ? d.cursor.label
    : d instanceof Descend ? _get(d.cursor.parent, d.cursor.label)
    : bindMaybe(d.parent, clickedIDFromD) }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !line.children.find(child => !isSingleLine(child)), dText => true, dIdenticon => true, dList => dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), editorBehavior => isSingleLine(editorBehavior.child), guidEditor => isSingleLine(guidEditor.child), supportsUnderselection => isSingleLine(supportsUnderselection.child), label => isSingleLine(label.child), collapseToggle => true, button => true, placeholder => true, stringEditor => true, numberEditor => true) }

export class DComponent extends React.Component<DComponentProps, DComponentState> {
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
      mapMaybe(cursorFromD(this.props.d), cursor => focusEditorForCursor(document.body, cursor)) }
    return matchD(this.props.d,
      block => <span>{concatMap(block.children, (d, index) => d instanceof Block
        ? [<DComponent key={`block${index}`} ref={addChild} d={d} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} />]
        : [
          <br key={`br${index}`} />,
          <span key={"span" + index} style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />,
          <DComponent key={`d${index}`} ref={addChild} d={d} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} />])}</span>,
      line => <span>{line.children.map((d, index) => <DComponent ref={addChild} key={index} d={d} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} />)}</span>,
      dText => <span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dText.string}</span>,
      dIdenticon => <span className="identicon" onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}><IdenticonComponent guid={dIdenticon.guid} size={dIdenticon.size} /></span>,
      dList => {
        let collapseToggle = dList.collapseToggle ? <DComponent ref={addChild} d={dList.collapseToggle} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} /> : null
        let opening = <span onMouseDown={keepFocusForChooseID} onClick={selectOrChooseID}>{dList.opening}</span>
        let closing = <span>{dList.closing}</span>
        let singleLine = dList.children.length <= 1 && !dList.children.find(child => !isSingleLine(child))
        let activeListInsertion = this.state.activeListInsertion !== undefined && dList.insertionPoints[this.state.activeListInsertion] ? this.state.activeListInsertion : undefined
        let setActiveListInsertion = (i: number, active: boolean) => this.setState(({activeListInsertion}) => ({activeListInsertion: active ? i : activeListInsertion === i ? undefined : activeListInsertion}))
        let insertionPoint = (i: number, label: string) => dList.insertionPoints[i]
          ? <ListInsertionEditorComponent key={`insertion${i}`} ref={addChild} insertionPoint={dList.insertionPoints[i]} label={label} active={activeListInsertion === i} setActive={active => setActiveListInsertion(i, active)} scrollParent={this.props.scrollParent} runE={this.props.runE} />
          : <span key={`insertion${i}`}>{label}</span>
        let child = (d: D, i: number, depth: number) => <DComponent key={`child${i}`} ref={addChild} d={d} depth={depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} />
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
        let classNames = ["descend", ...maybeMap([[descend.unmatching, "unmatching"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
        return <span className={classNames.join(" ")} ref={span => { if (span) attachEditorDescend(span, descend) }}><DComponent ref={addChild} d={descend.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={descend.edgeContext} editorCommands={this.props.editorCommands} /></span> },
      editorBehavior => <DComponent ref={addChild} d={editorBehavior.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={mergeEditorCommands(this.props.editorCommands, editorBehavior.editorCommands)} />,
      guidEditor => <GuidEditorComponent ref={addChild} guidEditor={guidEditor} editorCommands={activeEditorCommands(this.props.edgeContext, this.props.editorCommands, guidEditor.editorCommands)} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      supportsUnderselection => <SupportsUnderselectionComponent ref={addChild} supportsUnderselection={supportsUnderselection} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} />,
      label => <span className="edgeLabel"><DComponent ref={addChild} d={label.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} edgeContext={this.props.edgeContext} editorCommands={this.props.editorCommands} /></span>,
      collapseToggle => <span className="collapseToggle" onClick={e => { e.stopPropagation(); this.props.runE(collapseToggle.action) }}>{collapseToggle.collapsed ? "▸" : "▾"}</span>,
      button => <input type="button" value={button.text} onClick={e => { e.stopPropagation(); this.props.runE(button.action) }} />,
      placeholderEditor => <PlaceholderEditorComponent ref={addChild} placeholderEditor={placeholderEditor} editorCommands={activeEditorCommands(this.props.edgeContext, this.props.editorCommands, placeholderEditor.editorCommands)} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      stringEditor => <StringEditorComponent ref={addChild} stringEditor={stringEditor} editorCommands={activeEditorCommands(this.props.edgeContext, this.props.editorCommands, stringEditor.editorCommands)} runE={this.props.runE} />,
      numberEditor => <NumberEditorComponent ref={addChild} numberEditor={numberEditor} editorCommands={activeEditorCommands(this.props.edgeContext, this.props.editorCommands, numberEditor.editorCommands)} runE={this.props.runE} /> )}}

type SupportsUnderselectionComponentState = {pendingEdgeLabel: boolean, missingLabel?: ID, focusMissingLabel?: boolean}

class SupportsUnderselectionComponent extends React.Component<{supportsUnderselection: SupportsUnderselection, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, edgeContext?: EdgeContext, editorCommands?: EditorCommands}, SupportsUnderselectionComponentState> {
  state: SupportsUnderselectionComponentState = {pendingEdgeLabel: false}
  span: HTMLSpanElement | null
  child: Maybe<DComponent> = nothing
  missingChild: Maybe<DComponent> = nothing
  pendingInput: PlaceholderInputComponent | null
  labelEditorState: PlaceholderEditorState = {}
  onScroll() {
    if (this.child) this.child.onScroll()
    if (this.missingChild) this.missingChild.onScroll()
    if (this.pendingInput) this.pendingInput.onScroll() }
  activeState(): PlaceholderEditorActiveState {
    return {
      entries: buildEdgeLabelEntries(id => this.chooseLabel(id())),
      editorState: this.labelEditorState }}
  startNewEdge() {
    this.labelEditorState = {}
    this.setState({pendingEdgeLabel: true, missingLabel: undefined, focusMissingLabel: false}) }
  chooseLabel(label: ID) {
    this.labelEditorState = {}
    this.setState({pendingEdgeLabel: false, missingLabel: label, focusMissingLabel: true}) }
  missingField(label: ID) {
    const {cursor, id} = this.props.supportsUnderselection
    const edgeContext = edgeContextFromEdge({parent: id, label}, typeFromEdge({parent: id, label}))
    return renderField(cursor, id, label, {
      ...edgeContext,
      commit: id => {
        mapMaybe(edgeContext.commit, commit => commit(id))
        this.setState({missingLabel: undefined, focusMissingLabel: false}) }}) }
  render() {
    let editorCommands = mergeEditorCommands(this.props.editorCommands, {newEdge: () => this.startNewEdge()})
    return <span ref={span => { this.span = span }}>
      <DComponent
        ref={dComponent => { this.child = dComponent || nothing }}
        d={this.props.supportsUnderselection.child}
        depth={this.props.depth}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE}
        edgeContext={this.props.edgeContext}
        editorCommands={editorCommands} />
      {this.state.pendingEdgeLabel
        ? <span>
          <br />
          <span style={{width: indentWidth * (this.props.depth + 1) + "px", display: "inline-block"}} />
          <PlaceholderInputComponent
            ref={pendingInput => { this.pendingInput = pendingInput }}
            activeState={this.activeState()}
            placeholder="label"
            editorCommands={{commit: id => mapMaybe(id, id => this.chooseLabel(id))}}
            cursor={this.props.supportsUnderselection.cursor}
            descend={descendFromD(this.props.supportsUnderselection)}
            scrollParent={this.props.scrollParent}
            runE={this.props.runE}
            closeCompletion={() => {
              this.labelEditorState.completionOpen = false
              this.labelEditorState.value = ""
              this.labelEditorState.itemSelection = nothing
              this.forceUpdate() }}
            cancel={() => this.setState({pendingEdgeLabel: false})}
            blur={() => this.setState({pendingEdgeLabel: false})}
            commit={(action, e) => {
              e.preventDefault()
              e.stopPropagation()
              action() }} />
          <span> →</span>
        </span>
        : null}
      {mapMaybe(this.state.missingLabel, label => <DComponent
        key="missingLabel"
        ref={dComponent => { this.missingChild = dComponent || nothing }}
        d={this.missingField(label)}
        depth={this.props.depth}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE}
        edgeContext={this.props.edgeContext}
        editorCommands={this.props.editorCommands} />)}
    </span> }
  componentDidUpdate() {
    if (this.state.focusMissingLabel && this.span)
      mapMaybe(this.state.missingLabel, label => {
        focusEditorForCursor(this.span!, _childCursor(this.props.supportsUnderselection.cursor, this.props.supportsUnderselection.id, label))
        this.setState({focusMissingLabel: false}) }) }}

class GuidEditorComponent extends React.Component<{guidEditor: GuidEditor, editorCommands: EditorCommands, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  span: HTMLSpanElement | null
  child: Maybe<DComponent> = nothing
  onScroll() { if (this.child) this.child.onScroll() }
  attachEditorCommands() {
    if (this.span) {
      attachEditorCommands(this.span, this.props.editorCommands)
      attachEditorFocus(this.span, {cursor: this.props.guidEditor.cursor, descend: descendFromD(this.props.guidEditor), focusWhenSelected: this.props.guidEditor.focusWhenSelected}) }}
  render() {
    let childEditorCommands = {...this.props.editorCommands, commit: undefined}
    return <span
      className="guidEditor"
      tabIndex={0}
      onMouseDown={e => { if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) e.preventDefault() }}
      onClick={e => { e.stopPropagation(); focus(e.currentTarget) }}
      onKeyDown={e => mapMaybe(editorKeyDownAction(this.props.editorCommands, e), action => this.props.runE(action))}
      ref={span => { this.span = span }} >
      <DComponent
        ref={dComponent => { this.child = dComponent || nothing }}
        d={this.props.guidEditor.child}
        depth={this.props.depth}
        scrollParent={this.props.scrollParent}
        runE={this.props.runE}
        edgeContext={undefined}
        editorCommands={childEditorCommands} />
    </span> }
  componentDidMount() { this.attachEditorCommands() }
  componentDidUpdate() { this.attachEditorCommands() }
  componentWillUnmount() { if (this.span) { detachEditorCommands(this.span); detachEditorFocus(this.span) } } }
