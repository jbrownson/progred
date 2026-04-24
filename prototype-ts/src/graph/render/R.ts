import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { cursorHasCycle } from "../cursor/cursorHasCycle"
import { Button, D, Descend } from "./D"
import { environment, get, SourceID } from "../Environment"
import { ID } from "../model/ID"
import { selectionStateFromCursor } from "../editor/selectionIfSelected"
import { getCollapsed, setCollapsed } from "../editor/setCollapsed"
import { typeFromCursor } from "../cursor/typeFromCursor"
import { typeMatches } from "../typeMatches"

export type Render = (cursor: Cursor, id: Maybe<SourceID>) => Maybe<D>

export const alwaysFail: Render = () => nothing
export function dispatch(...renders: Render[]): Render {
  return (cursor, id) => renders.length === 0 ? nothing : altMaybe(renders[0](cursor, id), () => dispatch(...renders.slice(1))(cursor, id)) }

export type Change = undefined
export type Depedencies = Map/*TODO not actually Map*/<Change, Map<D, () => D>>

export function descend(cursor: Cursor, id: ID, label: ID, render = alwaysFail): D {
  let newCursor = _childCursor(cursor, id, label)
  if (fromMaybe(getCollapsed(newCursor), () => cursorHasCycle(newCursor))) return new Button("…", () => setCollapsed(newCursor, false))
  let newSourceID = get(id, label)
  return new Descend(newCursor, fromMaybe(render(newCursor, newSourceID), () => environment().defaultRender(newCursor, newSourceID)), selectionStateFromCursor(newCursor),
    fromMaybe(bindMaybe(newSourceID, newSourceID => bindMaybe(typeFromCursor(newCursor), type => mapMaybe(typeMatches(newSourceID.id, type), typeMatches => !typeMatches))), () => false) )}
