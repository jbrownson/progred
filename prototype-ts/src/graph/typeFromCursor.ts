import { bindMaybe, Maybe, nothing } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { Field, headField, ListType, tailField, Type } from "./graph"

function findListStart(cursor: Maybe<Cursor>): Maybe<Cursor> {
  return bindMaybe(cursor, cursor => cursor.label === tailField.id ? findListStart(cursor.parentCursor) : cursor) }

function listNestedType(cursor: Maybe<Cursor>): Maybe<Type> {
  return bindMaybe(cursor, cursor =>
    cursor.label === headField.id
      ? bindMaybe(listNestedType(findListStart(cursor.parentCursor)), type =>
          type instanceof ListType
            ? type.type
            : nothing)
      : bindMaybe(Field.fromID(cursor.label), field => field.type)) }

export function typeFromCursor(cursor: Cursor): Maybe<Type> { return listNestedType(cursor) }