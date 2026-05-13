import * as React from "react"
import { mapMaybe, maybeMap, Maybe, nothing } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { attachEditorCommands, detachEditorCommands, EdgeContext, EditorCommands, editorKeyDownAction } from "../editor/EditorCommands"
import { Entry } from "../editor/Entry"
import { Match } from "../editor/filters"
import { attachEditorDescend, attachEditorFocus, detachEditorFocus, focusEditorForCursor } from "../editor/EditorFocus"
import { focus } from "../editor/ignoreFocusEvents"
import { _get, Environment, environment, withEnvironment } from "../Environment"
import { GUID, ID, NID, SID } from "../model/ID"
import { NumberEditorComponent } from "../components/NumberEditorComponent"
import { PlaceholderEditorComponent } from "../components/PlaceholderEditorComponent"
import { PlaceholderInputComponent } from "../components/PlaceholderInputComponent"
import { StringEditorComponent } from "../components/StringEditorComponent"
import { activeEditorCommands, childContext, D, isSingleLine, mergeEditorCommands, ProjectionContext, projectionElement, ProjectionScope } from "./ProjectionContext"
import { indentWidth } from "./ProjectionLayout"

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
      descend
    })}>{props.child}</ProjectionScope>
  </span>
}

export function guidEditor(cursor: Cursor, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands = {}): D {
  return projectionElement(GuidEditorComponent, {cursor, id, child, focusWhenSelected, editorCommands, rootEditorCommands}, "guidEditor", isSingleLine(child))
}

function GuidEditorComponent(props: {cursor: Cursor, id: GUID, child: D, focusWhenSelected: boolean, editorCommands: EditorCommands, rootEditorCommands: EditorCommands}) {
  const context = React.useContext(ProjectionContext)
  const span = React.useRef<HTMLSpanElement | null>(null)
  const editorCommands = () => mergeEditorCommands(activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), props.rootEditorCommands)
  let childEditorCommands = {...activeEditorCommands(context.edgeContext, context.editorCommands, props.editorCommands), commit: undefined}
  React.useLayoutEffect(() => {
    let element = span.current
    if (!element) return
    attachEditorCommands(element, editorCommands())
    attachEditorFocus(element, {cursor: props.cursor, descend: context.descend, focusWhenSelected: props.focusWhenSelected})
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
    <ProjectionScope context={childContext(context, {
      edgeContext: undefined,
      editorCommands: childEditorCommands })}>{props.child}</ProjectionScope>
  </span>
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

export function supportsUnderselection(cursor: Cursor, id: ID, child: D, missingField: (label: ID) => D): D {
  return projectionElement(SupportsUnderselectionComponent, {cursor, id, child, missingField, environment: environment()}, "supportsUnderselection", isSingleLine(child))
}

type SupportsUnderselectionComponentState = {pendingEdgeLabel: boolean, missingLabel?: ID, focusMissingLabel?: boolean}

function SupportsUnderselectionComponent(props: {cursor: Cursor, id: ID, child: D, missingField: (label: ID) => D, environment: Environment}) {
  const context = React.useContext(ProjectionContext)
  const [state, setState] = React.useState<SupportsUnderselectionComponentState>({pendingEdgeLabel: false})
  const [, forceUpdate] = React.useReducer(n => n + 1, 0)
  const span = React.useRef<HTMLSpanElement | null>(null)
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
    if (state.focusMissingLabel && span.current)
      mapMaybe(state.missingLabel, label => {
        focusEditorForCursor(span.current!, _childCursor(props.cursor, props.id, label))
        setState(state => ({...state, focusMissingLabel: false})) }) })
  return <span ref={span}>
    <ProjectionScope context={childContext(context, {editorCommands})}>{props.child}</ProjectionScope>
    {state.pendingEdgeLabel
      ? <span>
        <br />
        <span style={{width: indentWidth * (context.depth + 1) + "px", display: "inline-block"}} />
        <PlaceholderInputComponent
          activeState={activeState()}
          placeholder="label"
          editorCommands={{commit: id => mapMaybe(id, id => chooseLabel(id))}}
          cursor={props.cursor}
          descend={context.descend}
          scrollParent={context.scrollParent}
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
      <ProjectionScope key="missingLabel" context={context}>{withEnvironment(props.environment, () => props.missingField(label))}</ProjectionScope>)}
  </span>
}

export function label(cursor: Cursor, child: D): D {
  return projectionElement(LabelComponent, {cursor, child}, "label", isSingleLine(child))
}

function LabelComponent(props: {cursor: Cursor, child: D}) {
  const context = React.useContext(ProjectionContext)
  return <span className="edgeLabel"><ProjectionScope context={childContext(context, {
    chooseID: () => props.cursor.label,
    focusCursor: props.cursor })}>{props.child}</ProjectionScope></span>
}
