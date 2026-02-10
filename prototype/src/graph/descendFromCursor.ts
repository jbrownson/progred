import { altMaybe, bindMaybe, firstMaybe, Maybe } from "../lib/Maybe"
import { Cursor, cursorsEqual } from "./Cursor"
import { D, Descend } from "./D"

export function descendFromCursor(rootDescend: Descend, viewsDescend: Maybe<Descend>, cursor: Cursor) {
  return altMaybe(_descendFromCursor(rootDescend, cursor), () => bindMaybe(viewsDescend, viewsDescend => _descendFromCursor(viewsDescend, cursor))) }

function _descendFromCursor(d: D, cursor: Cursor): Maybe<Descend> {
  return d instanceof Descend && cursorsEqual(d.cursor, cursor) ? d : firstMaybe(d.children.map(child => () => _descendFromCursor(child, cursor))) }