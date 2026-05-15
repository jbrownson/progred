import * as React from "react"
import { mapMaybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { attachEditorCommands, detachEditorCommands, EdgeContext, EditorCommands, editorKeyDownAction } from "../editor/EditorCommands"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { attachEditorDescend, attachEditorFocus, detachEditorFocus, focusFirstEditor } from "../editor/EditorFocus"
import { focus } from "../editor/domFocus"
import { _get, Environment, environment, withEnvironment } from "../Environment"
import { Edge } from "../model/Edge"
import { GUID, ID } from "../model/ID"
import { NumberEditorComponent } from "../components/NumberEditorComponent"
import { PlaceholderEditorComponent } from "../components/PlaceholderEditorComponent"
import { PlaceholderInputComponent } from "../components/PlaceholderInputComponent"
import { StringEditorComponent } from "../components/StringEditorComponent"
import { activeEditorCommands, childContext, D, isSingleLine, mergeEditorCommands, DContext, dElement, DScope } from "./DContext"
import { indentWidth } from "./DLayout"

export function descendElement(edge: Edge, child: D, unmatching: boolean, edgeContext: EdgeContext = {}): D {
  return dElement(DescendComponent, {edge, child, unmatching, edgeContext}, "descend", isSingleLine(child))
}

function DescendComponent(props: {edge: Edge, child: D, unmatching: boolean, edgeContext: EdgeContext}) {
  const context = React.useContext(DContext)
  const descend = React.useMemo(() => ({edge: props.edge, edgeContext: props.edgeContext, unmatching: props.unmatching}), [props.edge, props.edgeContext, props.unmatching])
  let classNames = ["descend", ...maybeMap([[props.unmatching, "unmatching"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)]
  return <span className={classNames.join(" ")} ref={span => { if (span) attachEditorDescend(span, descend) }}>
    <DScope context={childContext(context, {
      edgeContext: props.edgeContext,
      chooseID: () => _get(props.edge.parent, props.edge.label),
      descend
    })}>{props.child}</DScope>
  </span>
}

export function guidEditor(edge: Edge, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands = {}): D {
  return dElement(GuidEditorComponent, {edge, id, child, focusWhenSelected, editorCommands, rootEditorCommands}, "guidEditor", isSingleLine(child))
}

function GuidEditorComponent(props: {edge: Edge, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands}) {
  const context = React.useContext(DContext)
  const span = React.useRef<HTMLSpanElement | null>(null)
  const editorCommands = () => mergeEditorCommands(activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), props.rootEditorCommands)
  let childEditorCommands = {...activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), commit: undefined}
  React.useLayoutEffect(() => {
    let element = span.current
    if (!element) return
    attachEditorCommands(element, editorCommands())
    attachEditorFocus(element, {edge: props.edge, descend: context.descend, focusWhenSelected: props.focusWhenSelected})
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
      editorCommands: childEditorCommands })}>{props.child}</DScope>
  </span>
}

export type PlaceholderEditorState = {value?: string, itemSelection?: number, entryListAbove?: boolean, completionOpen?: boolean}
export type PlaceholderEditorActiveState = {entries: (needle: string) => {a: Entry, matches: Match[]}[], editorState: PlaceholderEditorState}
export type PlaceholderEditor = {name: string, entries: (needle: string) => {a: Entry, matches: Match[]}[], activeState: Maybe<PlaceholderEditorActiveState>}
export function placeholderEditor(name: string, entries: (needle: string) => {a: Entry, matches: Match[]}[], activeState: Maybe<PlaceholderEditorActiveState>, editorCommands: EditorCommands): D {
  return dElement(PlaceholderEditorDComponent, {placeholderEditor: {name, entries, activeState}, editorCommands}, "placeholderEditor", true)
}

function PlaceholderEditorDComponent(props: {placeholderEditor: PlaceholderEditor, editorCommands: EditorCommands}) {
  const context = React.useContext(DContext)
  return <PlaceholderEditorComponent
    placeholderEditor={props.placeholderEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export type StringEditor = {string: string, writable: boolean}
export function stringEditor(string: string, writable: boolean, editorCommands: EditorCommands): D {
  return dElement(StringEditorDComponent, {stringEditor: {string, writable}, editorCommands}, "stringEditor", true)
}

function StringEditorDComponent(props: {stringEditor: StringEditor, editorCommands: EditorCommands}) {
  const context = React.useContext(DContext)
  return <StringEditorComponent
    stringEditor={props.stringEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export type NumberEditor = {number: number, writable: boolean}
export function numberEditor(number: number, writable: boolean, editorCommands: EditorCommands): D {
  return dElement(NumberEditorDComponent, {numberEditor: {number, writable}, editorCommands}, "numberEditor", true)
}

function NumberEditorDComponent(props: {numberEditor: NumberEditor, editorCommands: EditorCommands}) {
  const context = React.useContext(DContext)
  return <NumberEditorComponent
    numberEditor={props.numberEditor}
    editorCommands={activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands)}
    edge={context.descend?.edge}
    descend={context.descend}
    runE={context.runE} />
}

export function supportsUnderselection(edge: Edge, id: ID, child: D, missingField: (label: ID) => D): D {
  return dElement(SupportsUnderselectionComponent, {edge, id, child, missingField, environment: environment()}, "supportsUnderselection", isSingleLine(child))
}

type SupportsUnderselectionComponentState = {pendingEdgeLabel: boolean, missingLabel?: ID, focusMissingLabel?: boolean}

function SupportsUnderselectionComponent(props: {edge: Edge, id: ID, child: D, missingField: (label: ID) => D, environment: Environment}) {
  const context = React.useContext(DContext)
  const [state, setState] = React.useState<SupportsUnderselectionComponentState>({pendingEdgeLabel: false})
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
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
  React.useLayoutEffect(() => {
    if (state.focusMissingLabel && missingFieldSpan.current && focusFirstEditor(missingFieldSpan.current))
      setState(state => ({...state, focusMissingLabel: false})) })
  return <span>
    <DScope context={childContext(context, {editorCommands})}>{props.child}</DScope>
    {state.pendingEdgeLabel
      ? <span>
        <br />
        <span style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />
        <PlaceholderInputComponent
          activeState={activeState()}
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
    {mapMaybe(state.missingLabel, label =>
      <span key="missingLabel" ref={missingFieldSpan}><DScope context={context}>{withEnvironment(props.environment, () => props.missingField(label))}</DScope></span>)}
  </span>
}

export function label(edge: Edge, child: D): D {
  return dElement(LabelComponent, {edge, child}, "label", isSingleLine(child))
}

function LabelComponent(props: {edge: Edge, child: D}) {
  const context = React.useContext(DContext)
  return <span className="edgeLabel"><DScope context={childContext(context, {
    chooseID: () => props.edge.label })}>{props.child}</DScope></span>
}
