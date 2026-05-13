import { bindMaybe, booleanFromMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { buildEntries } from "../editor/buildEntries"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { cursorHasCycle } from "../cursor/cursorHasCycle"
import { Block, Collapsible, CollapseToggle, D, DIdenticon, DList, DText, GuidEditor, Label, Line, ListInsertionPoint, matchD, NumberEditor, PlaceholderEditor, StringEditor, SupportsUnderselection } from "./D"
import { _get, edges, environment, set, Source, SourceID, SourceType } from "../Environment"
import { Ctor, ctorField, EmptyList, GUIDEmptyList, Field, GUIDNonemptyList, HasID, headField, List, listFromID, ListType, matchList, nameField, NonemptyList, tailField } from "../graph"
import { GUID, guidFromID, ID, matchID, numberFromNID, SID } from "../model/ID"
import { stringFromID } from "../model/ID"
import { alwaysFail, descend, Render } from "./R"
import { copyResultForID } from "../editor/Copy"
import type { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { edgeContextFromCursor, edgeContextFromEdge } from "../editor/edgeContextFromCursor"
import { requestFocusForCursor, requestNextTabStopFromCursor } from "../editor/EditorFocus"

export function tryFirst(render: Render, defaultRender: (cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext) => D): (cursor: Cursor, id: Maybe<SourceID>, edgeContext?: EdgeContext) => D {
  return (cursor, sourceID, edgeContext) => fromMaybe(render(cursor, sourceID, edgeContext), () => defaultRender(cursor, sourceID, edgeContext)) }

function _defaultRender(cursor: Cursor, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext): D {
  edgeContext = fromMaybe(edgeContext, () => edgeContextFromCursor(cursor))
  return maybe(sourceID, () => renderNothing(cursor, edgeContext), sourceID => matchID(sourceID.id,
    guid => renderGUID(cursor, guid, sourceID.source),
    (sid, string) => renderString(cursor, sid, string, sourceID.source),
    number => renderNumber(cursor, number, sourceID.source) ))}

export const defaultRender = tryFirst(renderList(), _defaultRender)

function renderNothing(cursor: Cursor, edgeContext: EdgeContext): D {
  let entries = buildEntries(edgeContext.expectedType, id => mapMaybe(edgeContext.commit, commit => commit(id())))
  return new PlaceholderEditor(fromMaybe(bindMaybe(Field.fromID(cursor.label), field => field.name), () => "[unnamed]"), entries, nothing, placeholderEditorCommands(cursor)) }

export function editorCommands(cursor: Cursor, id: ID): EditorCommands {
  return {
    copy: () => ({referenceID: id, copyResult: copyResultForID(id)}) }}

function placeholderEditorCommands(cursor: Cursor): EditorCommands {
  return {keyDown: e => e.key === "[" ? mapMaybe(e.commit, commit => () => {
    e.preventDefault()
    e.stopPropagation()
    let tail = GUIDEmptyList.new()
    let newList = GUIDNonemptyList.new(id => ({id})).setTail(tail)
    commit(newList.id)
    requestFocusForCursor(_childCursor(cursor, newList.id, headField.id)) }) : nothing} }

export function renderDocumentGuidEditor(cursor: Cursor, sourceID: SourceID, d: D): D {
  let guid = guidFromID(sourceID.id)
  return sourceID.source.source === SourceType.DocumentType && guid !== undefined
    ? new SupportsUnderselection(cursor, guid, new GuidEditor(cursor, guid, d, true, editorCommands(cursor, guid)))
    : d }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !booleanFromMaybe(line.children.find(child => !isSingleLine(child))), dText => true, dIdenticon => true, dList => dList.children.length > 1 || !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), guidEditor => isSingleLine(guidEditor.child), supportsUnderselection => isSingleLine(supportsUnderselection.child), label => isSingleLine(label.child), collapsible => collapsible.singleLine, collapseToggle => true, button => true, placeholder => true, stringEditor => true, numberEditor => true )}

function renderIDLabel(id: ID): D {
  return matchID<D>(id,
    guid => fromMaybe<D>(mapMaybe(bindMaybe(_get(guid, nameField.id), stringFromID), name => new DText(name)), () => new DIdenticon(guid)),
    (sid, string) => new DText(`"${string}"`),
    nid => new DText(`${numberFromNID(nid)}`)) }

export function renderField(cursor: Cursor, id: ID, label: ID, edgeContext?: EdgeContext): D {
  let childD = descend(cursor, id, label, alwaysFail, edgeContext)
  let labelD = new Label(new Cursor(cursor, id, label), new Line(renderIDLabel(label), new DText(" →")) )
  return isSingleLine(childD)
    ? new Block(new Line(labelD, new DText(" "), childD))
    : new Block(labelD, new Block(childD)) }

function renderGUID(cursor: Cursor, guid: GUID, source: Source): D {
  let ctor = bindMaybe(_get(guid, ctorField.id), Ctor.fromID)
  let ctorFields = fromMaybe(bindMaybe(ctor, ctor => ctor.fields), () => [])
  let guidEdges = edges(guid)
  let writable = maybe(guidEdges, () => sourceIsWritable(source), ({source}) => sourceIsWritable(source))
  let extraLabels = maybe(guidEdges, () => [], ({edges}) => Array.from(edges.keys()).filter(edge => ctorFields.find(field => field.id === edge) === undefined))
  let hasName = booleanFromMaybe(ctorFields.find(field => field.id === nameField.id)) || booleanFromMaybe(extraLabels.find(label => label === nameField.id))
  let labels = [
    ...ctorFields.filter(field => field.id !== nameField.id && field.id !== ctorField.id).map(field => field.id),
    ...extraLabels.filter(label => label !== nameField.id && label !== ctorField.id) ]
  let defaultCollapsed = cursorHasCycle(cursor)
  let render = (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => {
    let nameDs = hasName
      ? collapsed
        ? maybe(bindMaybe(_get(guid, nameField.id), stringFromID), () => [], name => [new DText(" "), new DText(name)])
        : [new DText(" "), descend(cursor, guid, nameField.id)]
      : []
    let fieldDs = collapsed ? [] : labels.map(label => renderField(cursor, guid, label))
    const d = new Line(
      new GuidEditor(cursor, guid, fromMaybe<D>(bindMaybe(ctor, ctor => mapMaybe(ctor.name, name => new DText(name))), () => new DIdenticon(guid)),
        true,
        editorCommands(cursor, guid)),
      ...nameDs,
      ...(hasName || labels.length > 0 ? [new CollapseToggle(collapsed, () => setCollapsed(!collapsed))] : []),
      ...fieldDs )
    return writable ? new SupportsUnderselection(cursor, guid, d) : d }
  return defaultCollapsed || hasName || labels.length > 0 ? new Collapsible(defaultCollapsed, defaultCollapsed || labels.length === 0, render) : render(false, () => {}) }

function cursorsFromList<A extends HasID>(cursor: Cursor, edgeContext: EdgeContext, list: List<A>, visited = new Set<ID>()): Maybe<{nonemptys: {cursor: Cursor, edgeContext: EdgeContext, list: NonemptyList<A>}[], emptyListCursor: Cursor, emptyListEdgeContext: EdgeContext, emptyList: EmptyList}> {
  if (visited.has(list.id)) return nothing
  visited.add(list.id)
  return matchList(list,
    nonemptyList => bindMaybe(nonemptyList.tail, tail => {
      let tailCursor = _childCursor(cursor, nonemptyList.id, tailField.id)
      return mapMaybe(cursorsFromList(tailCursor, edgeContextFromEdge({parent: nonemptyList.id, label: tailField.id}, edgeContext.expectedType), tail, visited), ({nonemptys, emptyListCursor, emptyListEdgeContext, emptyList}) =>
        ({nonemptys: [{cursor, edgeContext, list: nonemptyList}, ...nonemptys], emptyListCursor, emptyListEdgeContext, emptyList}) )}),
    emptyList => ({nonemptys: [], emptyListCursor: cursor, emptyListEdgeContext: edgeContext, emptyList}) )}

function listElementType(edgeContext: EdgeContext) {
  return bindMaybe(edgeContext.expectedType, type => type instanceof ListType ? type.type : nothing) }

export function renderListParens(separator = ",", r = alwaysFail) { return renderList("(", ")", separator, r) }
export function renderListCurly(separator = ",", r = alwaysFail) { return renderList("{", "}", separator, r) }

export function renderList(opening = "[", closing = "]", separator = ",", r = alwaysFail): Render {
  return (listCursor, sourceID, listEdgeContext) => bindMaybe(sourceID, sourceID => {
    listEdgeContext = fromMaybe(listEdgeContext, () => edgeContextFromCursor(listCursor))
    return bindMaybe(listFromID(sourceID.id, id => ({id})), list =>
      mapMaybe(cursorsFromList(listCursor, listEdgeContext, list), ({nonemptys, emptyListCursor, emptyListEdgeContext, emptyList}) => {
        let defaultCollapsed = cursorHasCycle(listCursor)
        let render = (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => {
          let collapseToggle = list instanceof NonemptyList ? new CollapseToggle(collapsed, () => setCollapsed(!collapsed)) : nothing
          if (collapsed && collapseToggle) return renderDocumentGuidEditor(listCursor, sourceID, new DList(opening, [], closing, separator, collapseToggle))
          let insertionPoint = (cursor: Cursor, edgeContext: EdgeContext, oldTail: List<HasID>, requiresMeta = false): ListInsertionPoint => {
            let insert = (id: Maybe<ID>) => mapMaybe(edgeContext.commit, commit => {
              let newList = GUIDNonemptyList.new(id => ({id})).setTail(oldTail)
              mapMaybe(id, id => newList.setHead({id}))
              commit(newList.id)
              requestNextTabStopFromCursor(_childCursor(cursor, newList.id, headField.id)) })
            return {
              entries: buildEntries(listElementType(edgeContext), id => insert(id())),
              editorCommands: {commit: insert},
              requiresMeta }}
          let listItem = (cursor: Cursor, listEdgeContext: EdgeContext, list: NonemptyList<HasID>) => {
            let commit = (id: Maybe<ID>) => maybe(id,
              () => mapMaybe(list.tail, tail => mapMaybe(listEdgeContext.commit, commit => commit(tail.id))),
              id => mapMaybe(guidFromID(list.id), guid => set(guid, headField.id, id)) )
            return descend(cursor, list.id, headField.id, r, {commit, expectedType: listElementType(listEdgeContext)}) }
          let requiresMetaAfter = (list: NonemptyList<HasID>) => maybe(list.head, () => false, head => matchID(head.id, () => false, () => true, () => false))
          let insertionPoints = [
            ...nonemptys.map(({cursor, edgeContext, list}, i) => insertionPoint(cursor, edgeContext, list, i !== 0 && requiresMetaAfter(nonemptys[i - 1].list))),
            insertionPoint(emptyListCursor, emptyListEdgeContext, emptyList, maybe(nonemptys[nonemptys.length - 1], () => false, ({list}) => requiresMetaAfter(list))) ]
          return renderDocumentGuidEditor(listCursor, sourceID, new DList(opening, nonemptys.map(({cursor, edgeContext, list}) => listItem(cursor, edgeContext, list)), closing, separator, collapseToggle,
            insertionPoints)) }
        return defaultCollapsed || list instanceof NonemptyList ? new Collapsible(defaultCollapsed, defaultCollapsed, render) : render(false, () => {}) }))})}

function sourceIsWritable(source: Source) { return source.source === SourceType.DocumentType }

export function renderNumber(cursor: Cursor, number: number, source: Source): D {
  return new NumberEditor(number, number, sourceIsWritable(source), editorCommands(cursor, number)) }
export function renderString(cursor: Cursor, sid: SID, string: string, source: Source): D {
  return new StringEditor(sid, string, sourceIsWritable(source), editorCommands(cursor, sid)) }
