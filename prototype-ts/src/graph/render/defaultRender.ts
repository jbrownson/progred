import { bindMaybe, booleanFromMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { buildEntries } from "../editor/buildEntries"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { cursorHasCycle } from "../cursor/cursorHasCycle"
import { Block, CollapseToggle, D, DIdenticon, DList, DText, Label, Line, matchD, NumberEditor, Placeholder, StringEditor, SupportsUnderselection } from "./D"
import { _get, edges, environment, set, Source, SourceID, SourceType } from "../Environment"
import { Ctor, ctorField, EmptyList, Field, GUIDNonemptyList, HasID, headField, List, listFromID, matchList, nameField, NonemptyList, tailField } from "../graph"
import { GUID, guidFromID, ID, matchID, numberFromNID } from "../model/ID"
import { stringFromID } from "../model/ID"
import { alwaysFail, descend, Render } from "./R"
import { selectionIfSelected } from "../editor/selectionIfSelected"
import { getCollapsed, setCollapsed } from "../editor/setCollapsed"
import { typeFromCursor } from "../cursor/typeFromCursor"
import { selectedMissingLabels } from "./selectedMissingLabels"
import { pendingEdgeLabel } from "./pendingEdgeLabel"

export function tryFirst(render: Render, defaultRender: (cursor: Cursor, sourceID: Maybe<SourceID>) => D): (cursor: Cursor, id: Maybe<SourceID>) => D {
  return (cursor, sourceID) => fromMaybe(render(cursor, sourceID), () => defaultRender(cursor, sourceID)) }

function _defaultRender(cursor: Cursor, sourceID: Maybe<SourceID>): D {
  return maybe(sourceID, () => renderNothing(cursor), sourceID => matchID(sourceID.id,
    guid => renderGUID(cursor, guid, sourceID.source),
    (sid, string) => renderString(cursor, string, sourceID.source),
    number => renderNumber(cursor, number, sourceID.source) ))}

export const defaultRender = tryFirst(renderList(), _defaultRender)

function renderNothing(cursor: Cursor): D {
  let selectedState = bindMaybe(selectionIfSelected(cursor), selection =>
    ({entries: buildEntries(typeFromCursor(cursor), id => mapMaybe(guidFromID(cursor.parent), guid => set(guid, cursor.label, id()))), placeholderState: selection}) )
  return new Placeholder(fromMaybe(bindMaybe(Field.fromID(cursor.label), field => field.name), () => "[unnamed]"), selectedState) }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !booleanFromMaybe(line.children.find(child => !isSingleLine(child))), dText => true, dIdenticon => true, dList => dList.children.length > 1 || !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), supportsUnderselection => isSingleLine(supportsUnderselection.child), label => isSingleLine(label.child), collapseToggle => true, button => true, placeholder => true, stringEditor => true, numberEditor => true )}

function renderIDLabel(id: ID): D {
  return matchID<D>(id,
    guid => fromMaybe<D>(mapMaybe(bindMaybe(_get(guid, nameField.id), stringFromID), name => new DText(name)), () => new DIdenticon(guid)),
    (sid, string) => new DText(`"${string}"`),
    nid => new DText(`${numberFromNID(nid)}`)) }

export function renderField(cursor: Cursor, id: ID, label: ID): D {
  let childD = descend(cursor, id, label)
  let labelD = new Label(
    new Cursor(
      cursor, id, label,
      bindMaybe(cursor.sparseSpanningTree, sparseSpanningTree =>
        sparseSpanningTree.map.get(label) )), new Line(renderIDLabel(label), new DText(" →")) )
  return isSingleLine(childD)
    ? new Block(new Line(labelD, new DText(" "), childD))
    : new Block(labelD, new Block(childD)) }

function renderGUID(cursor: Cursor, guid: GUID, source: Source): D {
  let ctor = bindMaybe(_get(guid, ctorField.id), Ctor.fromID)
  let ctorFields = fromMaybe(bindMaybe(ctor, ctor => ctor.fields), () => [])
  let guidEdges = edges(guid)
  let writable = maybe(guidEdges, () => sourceIsWritable(source), ({source}) => sourceIsWritable(source))
  let extraLabels = maybe(guidEdges, () => [], ({edges}) => Array.from(edges.keys()).filter(edge => ctorFields.find(field => field.id === edge) === undefined))
  extraLabels = [...extraLabels, ...selectedMissingLabels(cursor, guid, [...ctorFields.map(field => field.id), ...extraLabels])]
  let hasName = booleanFromMaybe(ctorFields.find(field => field.id === nameField.id)) || booleanFromMaybe(extraLabels.find(label => label === nameField.id))
  let labels = [
    ...ctorFields.filter(field => field.id !== nameField.id && field.id !== ctorField.id).map(field => field.id),
    ...extraLabels.filter(label => label !== nameField.id && label !== ctorField.id) ]
  let pendingEdgeLabelDs = writable ? pendingEdgeLabel(cursor, guid) : []
  let collapsed = fromMaybe(getCollapsed(cursor), () => cursorHasCycle(cursor))
  let nameDs = hasName
    ? collapsed
      ? maybe(bindMaybe(_get(guid, nameField.id), stringFromID), () => [], name => [new DText(" "), new DText(name)])
      : [new DText(" "), descend(cursor, guid, nameField.id)]
    : []
  let fieldDs = collapsed ? [] : [...labels.map(label => renderField(cursor, guid, label)), ...pendingEdgeLabelDs]
  const d = new Line(
    fromMaybe<D>(bindMaybe(ctor, ctor => mapMaybe(ctor.name, name => new DText(name))), () => new DIdenticon(guid)),
    ...nameDs,
    ...(hasName || labels.length > 0 || pendingEdgeLabelDs.length > 0 ? [new CollapseToggle(collapsed, () => setCollapsed(cursor, !collapsed))] : []),
    ...fieldDs )
  return writable ? new SupportsUnderselection(d) : d }

function cursorsFromList<A extends HasID>(cursor: Cursor, list: List<A>, visited = new Set<ID>()): Maybe<{nonemptys: {cursor: Cursor, list: NonemptyList<A>}[], emptyListCursor: Cursor, emptyList: EmptyList}> {
  if (visited.has(list.id)) return nothing
  visited.add(list.id)
  return matchList(list,
    nonemptyList => bindMaybe(nonemptyList.tail, tail =>
      mapMaybe(cursorsFromList(_childCursor(cursor, nonemptyList.id, tailField.id), tail, visited), ({nonemptys, emptyListCursor, emptyList}) =>
        ({nonemptys: [{cursor, list: nonemptyList}, ...nonemptys], emptyListCursor, emptyList}) )),
    emptyList => ({nonemptys: [], emptyListCursor: cursor, emptyList}) )}

export function renderListParens(separator = ",", r = alwaysFail) { return renderList("(", ")", separator, r) }
export function renderListCurly(separator = ",", r = alwaysFail) { return renderList("{", "}", separator, r) }

export function renderList(opening = "[", closing = "]", separator = ",", r = alwaysFail): (listCursor: Cursor, sourceID: Maybe<SourceID>) => Maybe<D> {
  return (listCursor, sourceID) => {
    let list = bindMaybe(sourceID, sourceID => listFromID(sourceID.id, id => ({id})))
    let collapsed = fromMaybe(getCollapsed(listCursor), () => cursorHasCycle(listCursor))
    let collapseToggle = bindMaybe(list, list => list instanceof NonemptyList ? new CollapseToggle(collapsed, () => setCollapsed(listCursor, !collapsed)) : nothing)
    if (collapsed && collapseToggle) return new DList(opening, [], closing, separator, () => {}, collapseToggle)
    let cursors = bindMaybe(list, list => cursorsFromList(listCursor, list))
    return mapMaybe(cursors, ({emptyList, emptyListCursor, nonemptys}) => new DList(opening, nonemptys.map(({cursor, list}, i) => descend(cursor, list.id, headField.id, r)), closing, separator, i => i === nonemptys.length
      // TODO factor something out of the next two lines
      ? mapMaybe(guidFromID(emptyListCursor.parent), guid => { let newList = GUIDNonemptyList.new(id => ({id})).setTail(emptyList); set(guid, emptyListCursor.label, newList.id); environment().selection = {cursor: _childCursor(emptyListCursor, newList.id, headField.id)} })
      : bindMaybe(nonemptys[i], ({list, cursor}) => mapMaybe(guidFromID(cursor.parent), guid => { let newList = GUIDNonemptyList.new(id => ({id})).setTail(list); set(guid, cursor.label, newList.id); environment().selection = {cursor: _childCursor(cursor, newList.id, headField.id)} })), collapseToggle ))}}

function sourceIsWritable(source: Source) { return source.source === SourceType.DocumentType }

export function renderNumber(cursor: Cursor, number: number, source: Source): D {
  return new NumberEditor(number, mapMaybe(selectionIfSelected(cursor), selection => ({writable: sourceIsWritable(source), numberEditorState: selection}))) }
export function renderString(cursor: Cursor, string: string, source: Source): D {
  return new StringEditor(string, mapMaybe(selectionIfSelected(cursor), selection => ({writable: sourceIsWritable(source), stringEditorState: selection}))) }
