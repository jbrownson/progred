import { bindMaybe, Maybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { D, Descend, Label } from "./D"

export function cursorFromD(d: D): Maybe<Cursor> { return d instanceof Descend || d instanceof Label ? d.cursor : bindMaybe(d.parent, cursorFromD) }