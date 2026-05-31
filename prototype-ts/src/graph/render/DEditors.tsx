import * as React from "react"
import { mapMaybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { attachEditorCommands, detachEditorCommands, EdgeContext, EditorCommands, editorKeyDownAction } from "../editor/EditorCommands"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { attachEditorDescend, attachEditorFocus, detachEditorFocus, editorElementsForEdge, focusEditorFromElement, focusFirstEditor } from "../editor/EditorFocus"
import { focus } from "../editor/domFocus"
import { _get, withEnvironment } from "../Environment"
import { Edge } from "../model/Edge"
import { GUID, ID } from "../model/ID"
import { NumberEditorComponent } from "../components/NumberEditorComponent"
import { PlaceholderEditorComponent } from "../components/PlaceholderEditorComponent"
import { PlaceholderInputComponent } from "../components/PlaceholderInputComponent"
import { StringEditorComponent } from "../components/StringEditorComponent"
import { activeEditorCommands, childContext, D, isSingleLine, mergeEditorCommands, dElement, DScope, renderD, useDContext } from "./DContext"
import { indentWidth } from "./DLayout"

export function descendElement(edge: Edge, child: D, unmatching: boolean, edgeContext: EdgeContext = {}): D {
  return dElement(DescendComponent, {edge, child, unmatching, edgeContext}, {singleLine: isSingleLine(child)})
}

function DescendComponent(props: {edge: Edge, child: D, unmatching: boolean, edgeContext: EdgeContext}) {
  const context = useDContext()
  const descend = React.useMemo(() => ({edge: props.edge, edgeContext: props.edgeContext, unmatching: props.unmatching}), [props.edge, props.edgeContext, props.unmatching])
  const childID = withEnvironment(context.environment, () => _get(props.edge.parent, props.edge.label))
  const secondarySelected = context.secondarySelectionID !== undefined && childID === context.secondarySelectionID
  let classNames = ["descend", ...maybeMap([[props.unmatching, "unmatching"], [secondarySelected, "secondarySelected"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
  return <span className={classNames.join(" ")} ref={span => { if (span) attachEditorDescend(span, descend) }}>
    <DScope context={childContext(context, {
      edgeContext: props.edgeContext,
      chooseID: () => _get(props.edge.parent, props.edge.label),
      descend
    })}>{renderD(props.child)}</DScope>
  </span>
}

export function guidEditor(edge: Edge, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands = {}): D {
  return dElement(GuidEditorComponent, {edge, id, child, focusWhenSelected, editorCommands, rootEditorCommands}, {singleLine: isSingleLine(child)})
}

function GuidEditorComponent(props: {edge: Edge, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands}) {
  const context = useDContext()
  const span = React.useRef<HTMLSpanElement | null>(null)
  const editorCommands = () => mergeEditorCommands(activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), props.rootEditorCommands)
  let childEditorCommands = {...activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), commit: undefined}
  React.useLayoutEffect(() => {
    let element = span.current
    if (!element) return
    attachEditorCommands(element, editorCommands())
    attachEditorFocus(element, {id: props.id, edge: props.edge, descend: context.descend, focusWhenSelected: props.focusWhenSelected})
    return () => {
      detachEditorCommands(element)
      detachEditorFocus(element) }
  })
  return <span
    className="guidEditor"
    tabIndex={0}
    onMouseDown={e => { if (!(e.target instanceof HTMLInputElement) && !(e.target instanceof HTMLTextAreaElement)) e.preventDefault() }}
    onClick={e => { e.stopPropagation(); focus(e.currentTarget) }}
    onKeyDown={e => { if (e.target === e.currentTarget) mapMaybe(editorKeyDownAction(editorCommands(), e), action => context.runE(action)) }}
    ref={span} >
    <DScope context={childContext(context, {
      edgeContext: undefined,
      editorCommands: childEditorCommands })}>{renderD(props.child)}</DScope>
  </span>
}

export type PlaceholderEditorState = {value?: string, itemSelection?: number, entryListAbove?: boolean, completionOpen?: boolean}
export type EntryFilter = (needle: string) => {a: Entry, matches: Match[]}[]
export type PlaceholderEditorActiveState = {entries: EntryFilter, editorState: PlaceholderEditorState}
export type PlaceholderEditor = {name: string, entries: () => EntryFilter, activeState: Maybe<PlaceholderEditorActiveState>}
export function placeholderEditor(name: string, entries: () => EntryFilter, activeState: Maybe<PlaceholderEditorActiveState>, editorCommands: EditorCommands): D {
  return dElement(PlaceholderEditorDComponent, {placeholderEditor: {name, entries, activeState}, editorCommands}, {singleLine: true})
}

function PlaceholderEditorDComponent(props: {placeholderEditor: PlaceholderEditor, editorCommands: EditorCommands}) {
  const context = useDContext()
  return <PlaceholderEditorComponent
    placeholderEditor={{...props.placeholderEditor, entries: () => withEnvironment(context.environment, props.placeholderEditor.entries)}}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export type StringEditor = {id: ID, string: string, writable: boolean}
export function stringEditor(id: ID, string: string, writable: boolean, editorCommands: EditorCommands): D {
  return dElement(StringEditorDComponent, {stringEditor: {id, string, writable}, editorCommands}, {singleLine: true})
}

function StringEditorDComponent(props: {stringEditor: StringEditor, editorCommands: EditorCommands}) {
  const context = useDContext()
  return <StringEditorComponent
    stringEditor={props.stringEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export type NumberEditor = {number: number, writable: boolean}
export function numberEditor(number: number, writable: boolean, editorCommands: EditorCommands): D {
  return dElement(NumberEditorDComponent, {numberEditor: {number, writable}, editorCommands}, {singleLine: true})
}

function NumberEditorDComponent(props: {numberEditor: NumberEditor, editorCommands: EditorCommands}) {
  const context = useDContext()
  return <NumberEditorComponent
    numberEditor={props.numberEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export function supportsUnderselection(edge: Edge, id: ID, child: D, missingField: (label: ID) => D): D {
  return dElement(SupportsUnderselectionComponent, {edge, id, child, missingField}, {singleLine: isSingleLine(child)})
}

type SupportsUnderselectionComponentState = {pendingEdgeLabel: boolean, missingLabel?: ID, focusMissingLabel?: boolean}

function SupportsUnderselectionComponent(props: {edge: Edge, id: ID, child: D, missingField: (label: ID) => D}) {
  const context = useDContext()
  const [state, setState] = React.useState<SupportsUnderselectionComponentState>({pendingEdgeLabel: false})
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const span = React.useRef<HTMLSpanElement | null>(null)
  const missingFieldSpan = React.useRef<HTMLSpanElement | null>(null)
  const labelEditorState = React.useRef<PlaceholderEditorState>({})
  const chooseLabel = (label: ID) => {
    labelEditorState.current = {}
    setState({pendingEdgeLabel: false, missingLabel: label, focusMissingLabel: true}) }
  const activeState = (): PlaceholderEditorActiveState => ({
    entries: buildEdgeLabelEntries(id => chooseLabel(id())),
    editorState: labelEditorState.current })
  const startNewEdge = () => {
    labelEditorState.current = {}
    setState({pendingEdgeLabel: true, missingLabel: undefined, focusMissingLabel: false}) }
  let editorCommands = mergeEditorCommands(context.editorCommands, {newEdge: startNewEdge})
  const existingID = (label: ID) => withEnvironment(context.environment, () => _get(props.id, label))
  React.useLayoutEffect(() => {
    if (state.missingLabel !== undefined && existingID(state.missingLabel) !== nothing) {
      if (state.focusMissingLabel && span.current) {
        let editors = editorElementsForEdge(span.current, {parent: props.id, label: state.missingLabel})
        if (editors.length === 1) focusEditorFromElement(editors[0]) }
      setState({pendingEdgeLabel: false})
      return }
    if (state.focusMissingLabel && missingFieldSpan.current && focusFirstEditor(missingFieldSpan.current))
      setState(state => ({...state, focusMissingLabel: false})) })
  return <span ref={span}>
    <DScope context={childContext(context, {editorCommands})}>{renderD(props.child)}</DScope>
    {state.pendingEdgeLabel
      ? <span>
        <br />
        <span style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />
        <PlaceholderInputComponent
          activeState={withEnvironment(context.environment, activeState)}
          placeholder="label"
          editorCommands={{commit: id => mapMaybe(id, id => chooseLabel(id))}}
          descend={context.descend}
          runE={context.runE}
          closeCompletion={() => {
            labelEditorState.current.completionOpen = false
            labelEditorState.current.value = ""
            labelEditorState.current.itemSelection = nothing
            forceUpdate() }}
          cancel={() => setState({pendingEdgeLabel: false})}
          blur={() => setState({pendingEdgeLabel: false})}
          commit={(action, e) => {
            e.preventDefault()
            e.stopPropagation()
            action() }} />
        <span> →</span>
      </span>
      : null}
    {mapMaybe(state.missingLabel, label => existingID(label) !== nothing ? nothing :
      <span key="missingLabel" ref={missingFieldSpan}><DScope context={context}>{renderD(withEnvironment(context.environment, () => props.missingField(label)))}</DScope></span>)}
  </span>
}

export function label(edge: Edge, child: D): D {
  return dElement(LabelComponent, {edge, child}, {singleLine: isSingleLine(child)})
}

function LabelComponent(props: {edge: Edge, child: D}) {
  const context = useDContext()
  return <span className="edgeLabel"><DScope context={childContext(context, {
    chooseID: () => props.edge.label })}>{renderD(props.child)}</DScope></span>
}
