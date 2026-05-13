import * as React from "react"
import { concatMap, intersperse, join } from "../../lib/Array"
import { altMaybe, bindMaybe, mapMaybe, maybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { _get, Environment, environment, get, SourceID, SourceType, withEnvironment } from "../Environment"
import { rootField, viewsField } from "../graph"
import { GUID, ID, NID, SID } from "../model/ID"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { attachEditorCommands, commitIDToActiveElement, detachEditorCommands, editorKeyDownAction } from "../editor/EditorCommands"
import { attachEditorDescend, attachEditorFocus, detachEditorFocus, focusEditorForCursor } from "../editor/EditorFocus"
import { edgeContextFromCursor, edgeContextFromEdge } from "../editor/edgeContextFromCursor"
import { focus } from "../editor/ignoreFocusEvents"
import { chooseIDModifier } from "../editor/chooseIDModifier"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { _childCursor } from "../cursor/childCursor"
import { typeFromEdge } from "../typeFromEdge"
import { IdenticonComponent } from "../components/IdenticonComponent"
import { ListInsertionEditorComponent } from "../components/ListInsertionEditorComponent"
import { NumberEditorComponent } from "../components/NumberEditorComponent"
import { PlaceholderEditorComponent } from "../components/PlaceholderEditorComponent"
import { PlaceholderInputComponent } from "../components/PlaceholderInputComponent"
import { StringEditorComponent } from "../components/StringEditorComponent"
import { alwaysFail, Render } from "./R"
import { tryFirst } from "./defaultRender"

const indentWidth = 16

type ProjectionKind = "block" | "line" | "text" | "identicon" | "list" | "descend" | "guidEditor" | "supportsUnderselection" | "label" | "collapsible" | "collapseToggle" | "button" | "placeholderEditor" | "stringEditor" | "numberEditor"
type ProjectionProps = {projectionKind: ProjectionKind, projectionSingleLine: boolean} & Record<string, any>
export type Projection = React.ReactElement<ProjectionProps>
export type D = Projection

export type EditorDescend = {
  cursor: Cursor
  edgeContext: EdgeContext
  unmatching: boolean
}

type ProjectionContextValue = {
  depth: number
  scrollParent: () => HTMLElement | null
  runE: (f: () => void) => void
  edgeContext?: EdgeContext
  editorCommands?: EditorCommands
  chooseID?: () => Maybe<ID>
  focusCursor?: Cursor
  descend?: EditorDescend
  registerRootEditorCommands?: (commands: Maybe<EditorCommands>) => void
}

const ProjectionContext = React.createContext<ProjectionContextValue>({
  depth: 0,
  scrollParent: () => null,
  runE: f => f()
})

function projectionElement<P>(component: React.ComponentType<P>, props: P, kind: ProjectionKind, singleLine: boolean): D {
  return React.createElement(component, {...props, projectionKind: kind, projectionSingleLine: singleLine} as P & ProjectionProps) as D
}

export function projectionKind(d: D): ProjectionKind { return d.props.projectionKind }

export function isSingleLine(d: D): boolean { return d.props.projectionSingleLine }

function isBlock(d: D): boolean { return projectionKind(d) === "block" }

function mergeEditorCommands(parentCommands: Maybe<EditorCommands>, childCommands: EditorCommands): EditorCommands {
  let keyDown = parentCommands?.keyDown && childCommands.keyDown
    ? e => altMaybe(childCommands.keyDown!(e), () => parentCommands.keyDown!(e))
    : childCommands.keyDown || parentCommands?.keyDown
  return {
    ...parentCommands,
    ...childCommands,
    ...(keyDown ? {keyDown} : {}) }}

function activeEditorCommands(edgeContext: Maybe<EdgeContext>, inheritedCommands: Maybe<EditorCommands>, editorCommands: EditorCommands): EditorCommands {
  return {
    ...inheritedCommands,
    ...editorCommands,
    commit: edgeContext?.commit || editorCommands.commit || inheritedCommands?.commit }}

function childContext(context: ProjectionContextValue, props: Partial<ProjectionContextValue>): ProjectionContextValue {
  return {...context, ...props}
}

function ProjectionScope(props: {context: ProjectionContextValue, children: React.ReactNode}) {
  return <ProjectionContext.Provider value={props.context}>{props.children}</ProjectionContext.Provider>
}

export function block(...children: D[]): D {
  return projectionElement(BlockComponent, {children}, "block", false)
}

function BlockComponent(props: {children: D[]}) {
  const context = React.useContext(ProjectionContext)
  return <span>{concatMap(props.children, (d, index) => isBlock(d)
    ? [<ProjectionScope key={`block${index}`} context={childContext(context, {depth: context.depth + 1, registerRootEditorCommands: undefined})}>{d}</ProjectionScope>]
    : [
      <br key={`br${index}`} />,
      <span key={`indent${index}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
      <ProjectionScope key={`d${index}`} context={childContext(context, {depth: context.depth + 1, registerRootEditorCommands: undefined})}>{d}</ProjectionScope>])}</span>
}

export function line(...children: D[]): D {
  return projectionElement(LineComponent, {children}, "line", !children.find(child => !isSingleLine(child)))
}

function LineComponent(props: {children: D[]}) {
  const context = React.useContext(ProjectionContext)
  return <span>{props.children.map((d, index) =>
    <ProjectionScope key={index} context={childContext(context, {registerRootEditorCommands: undefined})}>{d}</ProjectionScope>)}</span>
}

export function dText(string: string): D {
  return projectionElement(TextComponent, {string}, "text", true)
}

function TextComponent(props: {string: string}) {
  const context = React.useContext(ProjectionContext)
  return <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.string}</span>
}

export function dIdenticon(guid: GUID, size = 16): D {
  return projectionElement(IdenticonProjectionComponent, {guid, size}, "identicon", true)
}

function IdenticonProjectionComponent(props: {guid: GUID, size: number}) {
  const context = React.useContext(ProjectionContext)
  return <span className="identicon" onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}><IdenticonComponent guid={props.guid} size={props.size} /></span>
}

function keepFocusForChooseID(e: React.MouseEvent) {
  if (chooseIDModifier(e)) {
    e.stopPropagation()
    e.preventDefault() }}

function selectOrChooseID(e: React.MouseEvent, context: ProjectionContextValue) {
  e.stopPropagation()
  if (chooseIDModifier(e)) {
    e.preventDefault()
    context.runE(() => maybe(context.chooseID?.(), () => false, commitIDToActiveElement))
    return }
  mapMaybe(context.focusCursor, cursor => focusEditorForCursor(document.body, cursor)) }

export type ListInsertionPoint = {
  entries: (needle: string) => {a: Entry, matches: Match[]}[],
  editorCommands: EditorCommands
  requiresMeta?: boolean }

export function dList(opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D> = nothing, insertionPoints: ListInsertionPoint[] = []): D {
  return projectionElement(ListComponent, {opening, children, closing, separator, collapseToggle, insertionPoints}, "list", children.length <= 1 && !children.find(child => !isSingleLine(child)))
}

function ListComponent(props: {opening: string, children: D[], closing: string, separator: string, collapseToggle: Maybe<D>, insertionPoints: ListInsertionPoint[]}) {
  const context = React.useContext(ProjectionContext)
  const [activeListInsertion, setActiveListInsertionState] = React.useState<number | undefined>(undefined)
  const activeInsertion = activeListInsertion !== undefined && props.insertionPoints[activeListInsertion] ? activeListInsertion : undefined
  const setActiveListInsertion = (i: number, active: boolean) => setActiveListInsertionState(activeListInsertion => active ? i : activeListInsertion === i ? undefined : activeListInsertion)
  context.registerRootEditorCommands?.(props.insertionPoints.length > 0
    ? {keyDown: e => e.key === "," ? () => {
        e.preventDefault()
        e.stopPropagation()
        setActiveListInsertion(0, true) } : nothing}
    : nothing)
  let opening = <span onMouseDown={e => keepFocusForChooseID(e)} onClick={e => selectOrChooseID(e, context)}>{props.opening}</span>
  let closing = <span>{props.closing}</span>
  let singleLine = props.children.length <= 1 && !props.children.find(child => !isSingleLine(child))
  let insertionPoint = (i: number, label: string) => props.insertionPoints[i]
    ? <ListInsertionEditorComponent key={`insertion${i}`} insertionPoint={props.insertionPoints[i]} label={label} active={activeInsertion === i} setActive={active => setActiveListInsertion(i, active)} scrollParent={context.scrollParent} runE={context.runE} />
    : <span key={`insertion${i}`}>{label}</span>
  let child = (d: D, i: number, depth: number) => {
    let insertionIndex = i + 1
    let editorCommands = props.insertionPoints[insertionIndex]
      ? mergeEditorCommands(context.editorCommands, {keyDown: e => e.key === "," && (e.metaKey || !props.insertionPoints[insertionIndex].requiresMeta) ? () => {
          e.preventDefault()
          e.stopPropagation()
          setActiveListInsertion(insertionIndex, true) } : nothing})
      : context.editorCommands
    return <ProjectionScope key={`child${i}`} context={childContext(context, {depth, editorCommands, registerRootEditorCommands: undefined})}>{d}</ProjectionScope> }
  let activeItems = (depth: number) => {
    let items: React.ReactNode[] = []
    for (let i = 0; i <= props.children.length; i++) {
      if (activeInsertion === i) items.push(insertionPoint(i, ""))
      if (i < props.children.length) items.push(child(props.children[i], i, depth)) }
    return items }
  let content = props.collapseToggle && (props.collapseToggle.props as unknown as CollapseToggleProps).collapsed
    ? [<span key="collapsed" className="collapsedListContents">...</span>]
    : activeInsertion !== undefined && singleLine
    ? [<span key="leading"> </span>, ...join(intersperse(activeItems(context.depth).map(item => [item]), i => [<span key={`separator${i}`}>{props.separator} </span>])), <span key="trailing"> </span>]
    : activeInsertion !== undefined
    ? join(activeItems(context.depth + 1).map((item, i, items) => [
        <br key={`br${i}`} />,
        <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
        item,
        i + 1 < items.length ? <span key={`separator${i}`}>{props.separator}</span> : null]))
    : singleLine
    ? [insertionPoint(0, " "), ...concatMap(props.children, (d, i) => [
      child(d, i, context.depth),
      insertionPoint(i + 1, " ")])]
    : [insertionPoint(0, " "), ...join(intersperse(
      props.children.map((d, i) => [
        <br key={`br${i}`} />,
        <span key={`indent${i}`} style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />,
        child(d, i, context.depth + 1),
        i === props.children.length - 1 ? insertionPoint(props.children.length, " ") : null]),
      i => [insertionPoint(i, props.separator)]))]
  return <span>{props.collapseToggle}{opening}{content}{closing}</span>
}

export function descendElement(cursor: Cursor, child: D, unmatching: boolean, edgeContext: EdgeContext = {}): D {
  return projectionElement(DescendComponent, {cursor, child, unmatching, edgeContext}, "descend", isSingleLine(child))
}

function DescendComponent(props: {cursor: Cursor, child: D, unmatching: boolean, edgeContext: EdgeContext}) {
  const context = React.useContext(ProjectionContext)
  const descend = React.useMemo(() => ({cursor: props.cursor, edgeContext: props.edgeContext, unmatching: props.unmatching}), [props.cursor, props.edgeContext, props.unmatching])
  let classNames = ["descend", ...maybeMap([[props.unmatching, "unmatching"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
  return <span className={classNames.join(" ")} ref={span => { if (span) attachEditorDescend(span, descend) }}>
    <ProjectionScope context={childContext(context, {
      edgeContext: props.edgeContext,
      chooseID: () => _get(props.cursor.parent, props.cursor.label),
      focusCursor: props.cursor,
      descend,
      registerRootEditorCommands: undefined })}>{props.child}</ProjectionScope>
  </span>
}

export function guidEditor(cursor: Cursor, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands): D {
  return projectionElement(GuidEditorComponent, {cursor, id, child, focusWhenSelected, editorCommands}, "guidEditor", isSingleLine(child))
}

class GuidEditorComponent extends React.Component<{cursor: Cursor, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands}, {}> {
  static contextType = ProjectionContext
  declare context: React.ContextType<typeof ProjectionContext>
  span: HTMLSpanElement | null
  rootEditorCommands: Maybe<EditorCommands> = nothing
  editorCommands() { return maybe(this.rootEditorCommands, () => activeEditorCommands(this.context.edgeContext, this.context.editorCommands, this.props.editorCommands), editorCommands => mergeEditorCommands(activeEditorCommands(this.context.edgeContext, this.context.editorCommands, this.props.editorCommands), editorCommands)) }
  attachEditorCommands() {
    if (this.span) {
      attachEditorCommands(this.span, this.editorCommands())
      attachEditorFocus(this.span, {cursor: this.props.cursor, descend: this.context.descend, focusWhenSelected: this.props.focusWhenSelected}) }}
  render() {
    this.rootEditorCommands = nothing
    let childEditorCommands = {...activeEditorCommands(this.context.edgeContext, this.context.editorCommands, this.props.editorCommands), commit: undefined}
    return <span
      className="guidEditor"
      tabIndex={0}
      onMouseDown={e => { if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) e.preventDefault() }}
      onClick={e => { e.stopPropagation(); focus(e.currentTarget) }}
      onKeyDown={e => { if (e.target === e.currentTarget) mapMaybe(editorKeyDownAction(this.editorCommands(), e), action => this.context.runE(action)) }}
      ref={span => { this.span = span }} >
      <ProjectionScope context={childContext(this.context, {
        edgeContext: undefined,
        editorCommands: childEditorCommands,
        registerRootEditorCommands: commands => { this.rootEditorCommands = commands } })}>{this.props.child}</ProjectionScope>
    </span> }
  componentDidMount() { this.attachEditorCommands() }
  componentDidUpdate() { this.attachEditorCommands() }
  componentWillUnmount() { if (this.span) { detachEditorCommands(this.span); detachEditorFocus(this.span) } } }

export function supportsUnderselection(cursor: Cursor, id: ID, child: D, missingField: (label: ID) => D): D {
  return projectionElement(SupportsUnderselectionComponent, {cursor, id, child, missingField, environment: environment()}, "supportsUnderselection", isSingleLine(child))
}

type SupportsUnderselectionComponentState = {pendingEdgeLabel: boolean, missingLabel?: ID, focusMissingLabel?: boolean}

class SupportsUnderselectionComponent extends React.Component<{cursor: Cursor, id: ID, child: D, missingField: (label: ID) => D, environment: Environment}, SupportsUnderselectionComponentState> {
  static contextType = ProjectionContext
  declare context: React.ContextType<typeof ProjectionContext>
  state: SupportsUnderselectionComponentState = {pendingEdgeLabel: false}
  span: HTMLSpanElement | null
  pendingInput: PlaceholderInputComponent | null
  labelEditorState = {} as PlaceholderEditorState
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
  render() {
    let editorCommands = mergeEditorCommands(this.context.editorCommands, {newEdge: () => this.startNewEdge()})
    return <span ref={span => { this.span = span }}>
      <ProjectionScope context={childContext(this.context, {editorCommands})}>{this.props.child}</ProjectionScope>
      {this.state.pendingEdgeLabel
        ? <span>
          <br />
          <span style={{width: indentWidth * (this.context.depth + 1) + "px", display: "inline-block"}} />
          <PlaceholderInputComponent
            ref={pendingInput => { this.pendingInput = pendingInput }}
            activeState={this.activeState()}
            placeholder="label"
            editorCommands={{commit: id => mapMaybe(id, id => this.chooseLabel(id))}}
            cursor={this.props.cursor}
            descend={this.context.descend}
            scrollParent={this.context.scrollParent}
            runE={this.context.runE}
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
      {mapMaybe(this.state.missingLabel, label =>
        <ProjectionScope key="missingLabel" context={this.context}>{withEnvironment(this.props.environment, () => this.props.missingField(label))}</ProjectionScope>)}
    </span> }
  componentDidUpdate() {
    if (this.state.focusMissingLabel && this.span)
      mapMaybe(this.state.missingLabel, label => {
        focusEditorForCursor(this.span!, _childCursor(this.props.cursor, this.props.id, label))
        this.setState({focusMissingLabel: false}) }) }}

export function label(cursor: Cursor, child: D): D {
  return projectionElement(LabelComponent, {cursor, child}, "label", isSingleLine(child))
}

function LabelComponent(props: {cursor: Cursor, child: D}) {
  const context = React.useContext(ProjectionContext)
  return <span className="edgeLabel"><ProjectionScope context={childContext(context, {
    chooseID: () => props.cursor.label,
    focusCursor: props.cursor,
    registerRootEditorCommands: undefined })}>{props.child}</ProjectionScope></span>
}

export function collapsible(defaultCollapsed: boolean, singleLine: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D): D {
  return projectionElement(CollapsibleComponent, {defaultCollapsed, render, environment: environment()}, "collapsible", singleLine)
}

function CollapsibleComponent(props: {defaultCollapsed: boolean, render: (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D, environment: Environment}) {
  const [collapsed, setCollapsed] = React.useState(props.defaultCollapsed)
  const context = React.useContext(ProjectionContext)
  let editorCommands = mergeEditorCommands(context.editorCommands, {collapse: () => setCollapsed(true)})
  return <ProjectionScope context={childContext(context, {editorCommands})}>{withEnvironment(props.environment, () => props.render(collapsed, setCollapsed))}</ProjectionScope>
}

type CollapseToggleProps = {collapsed: boolean, action: () => void}
export function collapseToggle(collapsed: boolean, action: () => void): D {
  return projectionElement(CollapseToggleComponent, {collapsed, action}, "collapseToggle", true)
}

function CollapseToggleComponent(props: CollapseToggleProps) {
  return <span className="collapseToggle" onClick={e => { e.stopPropagation(); props.action() }}>{props.collapsed ? "▸" : "▾"}</span>
}

export function button(text: string, action: () => void): D {
  return projectionElement(ButtonComponent, {text, action}, "button", true)
}

function ButtonComponent(props: {text: string, action: () => void}) {
  const context = React.useContext(ProjectionContext)
  return <input type="button" value={props.text} onClick={e => { e.stopPropagation(); context.runE(props.action) }} />
}

export type PlaceholderEditorState = {value?: string, itemSelection?: number, entryListAbove?: boolean, completionOpen?: boolean}
export type PlaceholderEditorActiveState = {entries: (needle: string) => {a: Entry, matches: Match[]}[], editorState: PlaceholderEditorState}
export type PlaceholderEditor = {name: string, entries: (needle: string) => {a: Entry, matches: Match[]}[], activeState: Maybe<PlaceholderEditorActiveState>, editorCommands: EditorCommands}
export function placeholderEditor(name: string, entries: (needle: string) => {a: Entry, matches: Match[]}[], activeState: Maybe<PlaceholderEditorActiveState>, editorCommands: EditorCommands): D {
  return projectionElement(PlaceholderEditorProjectionComponent, {placeholderEditor: {name, entries, activeState, editorCommands}}, "placeholderEditor", true)
}

function PlaceholderEditorProjectionComponent(props: {placeholderEditor: PlaceholderEditor}) {
  const context = React.useContext(ProjectionContext)
  return <PlaceholderEditorComponent
    placeholderEditor={props.placeholderEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.placeholderEditor.editorCommands)}
    cursor={context.focusCursor}
    descend={context.descend}
    scrollParent={context.scrollParent}
    runE={context.runE} />
}

export type StringEditor = {id: SID, string: string, writable: boolean, editorCommands: EditorCommands}
export function stringEditor(id: SID, string: string, writable: boolean, editorCommands: EditorCommands): D {
  return projectionElement(StringEditorProjectionComponent, {stringEditor: {id, string, writable, editorCommands}}, "stringEditor", true)
}

function StringEditorProjectionComponent(props: {stringEditor: StringEditor}) {
  const context = React.useContext(ProjectionContext)
  return <StringEditorComponent
    stringEditor={props.stringEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.stringEditor.editorCommands)}
    cursor={context.focusCursor}
    descend={context.descend}
    runE={context.runE} />
}

export type NumberEditor = {id: NID, number: number, writable: boolean, editorCommands: EditorCommands}
export function numberEditor(id: NID, number: number, writable: boolean, editorCommands: EditorCommands): D {
  return projectionElement(NumberEditorProjectionComponent, {numberEditor: {id, number, writable, editorCommands}}, "numberEditor", true)
}

function NumberEditorProjectionComponent(props: {numberEditor: NumberEditor}) {
  const context = React.useContext(ProjectionContext)
  return <NumberEditorComponent
    numberEditor={props.numberEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.numberEditor.editorCommands)}
    cursor={context.focusCursor}
    descend={context.descend}
    runE={context.runE} />
}

export function createProjection(r: Render = alwaysFail) {
  let rootCursor = new Cursor(nothing, environment().rootViews.id, rootField.id)
  let rootSourceID = mapMaybe(environment().rootViews.root, ({id}) =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().rootViews.id}}))
  let rootDescend = descendElement(rootCursor, tryFirst(r, environment().defaultRender)(rootCursor, rootSourceID, edgeContextFromCursor(rootCursor)), false, edgeContextFromCursor(rootCursor))
  let viewsCursor = new Cursor(nothing, environment().rootViews.id, viewsField.id)
  let viewsDescend = mapMaybe(get(environment().rootViews.id, viewsField.id), viewsSourceID =>
    descendElement(viewsCursor, environment().defaultRender(viewsCursor, viewsSourceID, edgeContextFromCursor(viewsCursor)), false, edgeContextFromCursor(viewsCursor)))
  return {rootDescend, viewsDescend} }

export function ProjectionRoot(props: {d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, edgeContext?: EdgeContext, editorCommands?: EditorCommands}) {
  return <ProjectionContext.Provider value={{
    depth: props.depth,
    scrollParent: props.scrollParent,
    runE: props.runE,
    edgeContext: props.edgeContext,
    editorCommands: props.editorCommands
  }}>{props.d}</ProjectionContext.Provider>
}
