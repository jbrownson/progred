import { bindMaybe, booleanFromMaybe, mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { _childCursor } from "./childCursor"
import { Cursor } from "./Cursor"
import { _delete, _get, documentSourceFromSource, environment, get, set, Source, SourceType } from "./Environment"
import { ctorField, emptyListCtor, GUIDEmptyList, GUIDNonemptyList, headField, listFromID, nonemptyListCtor, tailField } from "./graph"
import { guidFromID } from "./ID"

function doAppend(cursor: Cursor): Maybe<Cursor> {
  return bindMaybe(bindMaybe(_get(cursor.parent, cursor.label), tailID => listFromID(tailID, id => ({id}))), oldTail =>
    mapMaybe(guidFromID(cursor.parent), parent => {
      let newList = GUIDNonemptyList.new(id => ({id})).setTail(oldTail)
      set(parent, cursor.label, newList.id)
      return _childCursor(cursor, newList.id, headField.id) }))}

export function insertBeforeListElemCursor(cursor: Cursor): Maybe<Cursor> {
  return cursor.label === headField.id && _get(cursor.parent, ctorField.id) === nonemptyListCtor.id
    ? doAppend(cursor)
    : nothing }

export function setCursorToEmptyList(cursor: Cursor): Maybe<Cursor> {
  if (_get(cursor.parent, cursor.label) === nothing) {
    return mapMaybe(guidFromID(cursor.parent), parent => {
      let newList = GUIDEmptyList.new()
      set(parent, cursor.label, newList.id)
      return cursor }) }
  return nothing }

export function insertAfterListElemCursor(cursor: Cursor): Maybe<Cursor> {
  return cursor.label === headField.id && _get(cursor.parent, ctorField.id) === nonemptyListCtor.id
    ? bindMaybe(mapMaybe(cursor.parentCursor, parentCursor => _childCursor(parentCursor, cursor.parent, tailField.id)), _parentCursor => bindMaybe(bindMaybe(_get(_parentCursor.parent, _parentCursor.label), tailID => listFromID(tailID, id => ({id}))), oldTail =>
      mapMaybe(guidFromID(_parentCursor.parent), parent => {
        let newList = GUIDNonemptyList.new(id => ({id})).setTail(oldTail)
        set(parent, _parentCursor.label, newList.id)
        return _childCursor(_parentCursor, newList.id, headField.id) })))
    : nothing }

export function appendToListCursor(cursor: Cursor): Maybe<Cursor> {
  return bindMaybe(_get(cursor.parent, cursor.label), dest => {
    let ctorID = _get(dest, ctorField.id)
    return ctorID === nonemptyListCtor.id
      ? appendToListCursor(_childCursor(cursor, dest, tailField.id))
      : ctorID === emptyListCtor.id
        ? doAppend(cursor)
        : nothing })}

export function deleteListElemCursor(cursor: Cursor): boolean {
  if (cursor.label === headField.id && _get(cursor.parent, ctorField.id) === nonemptyListCtor.id) {
    return booleanFromMaybe(bindMaybe(cursor.parentCursor, parentCursor =>
      bindMaybe(guidFromID(cursor.parent), parent =>
        bindMaybe(_get(cursor.parent, tailField.id), replacement =>
          bindMaybe(guidFromID(parentCursor.parent), parentParent => {
            let f = (source: Source) =>
              mapMaybe(documentSourceFromSource(source), source => {
                _delete(parent, cursor.label)
                set(parentParent, parentCursor.label, replacement)
                return {} })
            return maybe(get(parent, cursor.label), () => f({source: SourceType.DocumentType, guid: parent}), ({id, source}) => f(source))})))))}
  return false }

export function selectionCursorBindMaybe<A>(f: (cursor: Cursor) => Maybe<A>): Maybe<A> {
  let env = environment()
  return bindMaybe(env.selection, selection => f(selection.cursor)) }