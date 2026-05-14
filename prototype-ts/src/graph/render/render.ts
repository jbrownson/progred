import { bindMaybe, Maybe, nothing } from "../../lib/Maybe"
import type { D } from "./DContext"
import { _get } from "../Environment"
import { Ctor, ctorField } from "../graph"
import { ID } from "../model/ID"
import { Render } from "./R"

export function render0(ctor: Ctor, f: (id: ID) => D): Render {
  return (edge, sourceID, edgeContext, cyclePath) => renderByCtor(ctor, id => f(id) )(edge, sourceID, edgeContext, cyclePath) }

export function renderByCtor(ctor: Ctor, f: (id: ID) => Maybe<D>): Render {
  return (_edge, sourceID) => bindMaybe(sourceID, sourceID => bindMaybe(_get(sourceID.id, ctorField.id), ctorID => ctorID === ctor.id ? f(sourceID.id) : nothing)) }
