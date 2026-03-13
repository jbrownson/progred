import { bindMaybe, booleanFromMaybe, fromMaybe, mapMaybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { _delete, documentSourceFromSource, get } from "./Environment"
import { guidFromID } from "./ID"
import { deleteListElemCursor, selectionCursorBindMaybe } from "./listCursorActions"

export function deleteCursorDefault(cursor: Cursor): boolean {
  return booleanFromMaybe(bindMaybe(guidFromID(cursor.parent), guid =>
      bindMaybe(get(guid, cursor.label), ({id, source}) =>
      mapMaybe(documentSourceFromSource(source), source => {
          _delete(guid, cursor.label)
          return {} }))))}

function composeDeletes(...deleters: ((cursor: Cursor) => boolean)[]) {
  return (cursor: Cursor): boolean => {
    return deleters.length === 0
      ? false
      : deleters[0](cursor) || composeDeletes(...deleters.slice(1))(cursor) } }

const deleteHandler = composeDeletes(deleteListElemCursor, deleteCursorDefault)

export function deleteCursor(cursor: Cursor): boolean { return deleteHandler(cursor) }

export function deleteSelection(): boolean { return fromMaybe(selectionCursorBindMaybe(deleteCursor), () => false) }