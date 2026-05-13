import { bindMaybe, Maybe } from "../../lib/Maybe"
import { Cursor } from "./Cursor"
import { D, Descend, GuidEditor, Label } from "../render/D"

export function cursorFromD(d: D): Maybe<Cursor> { return d instanceof Descend || d instanceof GuidEditor || d instanceof Label ? d.cursor : bindMaybe(d.parent, cursorFromD) }

export function descendFromD(d: D): Maybe<Descend> { return d instanceof Descend ? d : bindMaybe(d.parent, descendFromD) }
