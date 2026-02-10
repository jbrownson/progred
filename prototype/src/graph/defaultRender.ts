import { bindMaybe, booleanFromMaybe, fromMaybe, mapMaybe, Maybe, maybe } from "../lib/Maybe"
import { buildEntries } from "./buildEntries"
import { _childCursor } from "./childCursor"
import { Cursor } from "./Cursor"
import { Block, D, DList, DText, Label, Line, matchD, NumberEditor, Placeholder, StringEditor } from "./D"
import { _get, edges, environment, set, Source, SourceID, SourceType } from "./Environment"
import { Ctor, ctorField, EmptyList, Field, GUIDNonemptyList, HasID, headField, List, listFromID, matchList, nameField, NonemptyList, tailField } from "./graph"
import { GUID, guidFromID, ID, matchID } from "./ID"
import { stringFromID } from "./ID"
import { alwaysFail, descend, Render } from "./R"
import { selectionIfSelected } from "./selectionIfSelected"
import { typeFromCursor } from "./typeFromCursor"

export function tryFirst(render: Render, defaultRender: (cursor: Cursor, sourceID: Maybe<SourceID>) => D): (cursor: Cursor, id: Maybe<SourceID>) => D {
  return (cursor, sourceID) => fromMaybe(render(cursor, sourceID), () => defaultRender(cursor, sourceID)) }

function _defaultRender(cursor: Cursor, sourceID: Maybe<SourceID>): D {
  return maybe(sourceID, () => renderNothing(cursor), sourceID => matchID(sourceID.id,
    guid => renderGUID(cursor, guid),
    (sid, string) => renderString(cursor, string, sourceID.source),
    number => renderNumber(cursor, number, sourceID.source) ))}

export const defaultRender = tryFirst(renderList(), _defaultRender)

function renderNothing(cursor: Cursor): D {
  let selectedState = bindMaybe(selectionIfSelected(cursor), selection =>
    ({entries: buildEntries(typeFromCursor(cursor), id => mapMaybe(guidFromID(cursor.parent), guid => set(guid, cursor.label, id()))), placeholderState: selection}) )
  return new Placeholder(fromMaybe(bindMaybe(Field.fromID(cursor.label), field => field.name), () => "[unnamed]"), selectedState) }

function isSingleLine(d: D): boolean {
  return matchD(d, block => false, line => !booleanFromMaybe(line.children.find(child => !isSingleLine(child))), dText => true, dList => dList.children.length > 1 || !dList.children.find(child => !isSingleLine(child)),
    descend => isSingleLine(descend.child), label => isSingleLine(label.child), button => true, placeholder => true, stringEditor => true, numberEditor => true )}

export function renderField(cursor: Cursor, id: ID, label: ID): D {
  let childD = descend(cursor, id, label)
  let labelD = new Label(
    new Cursor(
      cursor, id, label,
      bindMaybe(cursor.sparseSpanningTree, sparseSpanningTree =>
        sparseSpanningTree.map.get(label) )), new Line(new DText(fromMaybe(bindMaybe(_get(label, nameField.id), stringFromID), () => "[unnamed]")), new DText(":")) )
  return isSingleLine(childD)
    ? new Block(new Line(labelD, new DText(" "), childD))
    : new Block(labelD, new Block(childD)) }

function renderGUID(cursor: Cursor, guid: GUID): D {
  let ctor = bindMaybe(_get(guid, ctorField.id), Ctor.fromID)
  let ctorFields = fromMaybe(bindMaybe(ctor, ctor => ctor.fields), () => [])
  let extraLabels = maybe(edges(guid), () => [], ({edges}) => Array.from(edges.keys()).filter(edge => ctorFields.find(field => field.id === edge) === undefined))
  return new Line(
    new DText(fromMaybe(mapMaybe(ctor, ctor => ctor.name), () => "[unnamed]")),
    ...booleanFromMaybe(ctorFields.find(field => field.id === nameField.id)) || booleanFromMaybe(extraLabels.find(label => label === nameField.id))
      ? [new DText(" "), descend(cursor, guid, nameField.id)]
      : [],
    ...ctorFields.filter(field => field.id !== nameField.id && field.id !== ctorField.id).map(field => renderField(cursor, guid, field.id)),
    ...extraLabels.filter(label => label !== nameField.id && label !== ctorField.id).map(label => renderField(cursor, guid, label)) )}

function cursorsFromList<A extends HasID>(cursor: Cursor, list: List<A>): Maybe<{nonemptys: {cursor: Cursor, list: NonemptyList<A>}[], emptyListCursor: Cursor, emptyList: EmptyList}> {
  return matchList(list,
    nonemptyList => bindMaybe(nonemptyList.tail, tail =>
      mapMaybe(cursorsFromList(_childCursor(cursor, nonemptyList.id, tailField.id), tail), ({nonemptys, emptyListCursor, emptyList}) =>
        ({nonemptys: [{cursor, list: nonemptyList}, ...nonemptys], emptyListCursor, emptyList}) )),
    emptyList => ({nonemptys: [], emptyListCursor: cursor, emptyList}) )}

export function renderListParens(separator = ",", r = alwaysFail) { return renderList("(", ")", separator, r) }
export function renderListCurly(separator = ",", r = alwaysFail) { return renderList("{", "}", separator, r) }

export function renderList(opening = "[", closing = "]", separator = ",", r = alwaysFail): (listCursor: Cursor, sourceID: Maybe<SourceID>) => Maybe<D> {
  return (listCursor, sourceID) => {
    let cursors = bindMaybe(sourceID, sourceID => bindMaybe(listFromID(sourceID.id, id => ({id})), list => cursorsFromList(listCursor, list)))
    return mapMaybe(cursors, ({emptyList, emptyListCursor, nonemptys}) => new DList(opening, nonemptys.map(({cursor, list}, i) => descend(cursor, list.id, headField.id, r)), closing, separator, i => i === nonemptys.length
      // TODO factor something out of the next two lines
      ? mapMaybe(guidFromID(emptyListCursor.parent), guid => { let newList = GUIDNonemptyList.new(id => ({id})).setTail(emptyList); set(guid, emptyListCursor.label, newList.id); environment().selection = {cursor: _childCursor(emptyListCursor, newList.id, headField.id)} })
      : bindMaybe(nonemptys[i], ({list, cursor}) => mapMaybe(guidFromID(cursor.parent), guid => { let newList = GUIDNonemptyList.new(id => ({id})).setTail(list); set(guid, cursor.label, newList.id); environment().selection = {cursor: _childCursor(cursor, newList.id, headField.id)} })) ))}}

function sourceIsWritable(source: Source) { return source.source === SourceType.DocumentType }

export function renderNumber(cursor: Cursor, number: number, source: Source): D {
  return new NumberEditor(number, mapMaybe(selectionIfSelected(cursor), selection => ({writable: sourceIsWritable(source), numberEditorState: selection}))) }
export function renderString(cursor: Cursor, string: string, source: Source): D {
  return new StringEditor(string, mapMaybe(selectionIfSelected(cursor), selection => ({writable: sourceIsWritable(source), stringEditorState: selection}))) }