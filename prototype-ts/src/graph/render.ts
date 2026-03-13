import { bindMaybe, Maybe, nothing } from "../lib/Maybe"
import { D } from "./D"
import { _get } from "./Environment"
import { Ctor, ctorField, Field } from "./graph"
import { ID } from "./ID"
import { alwaysFail, descend, Render } from "./R"

// TODO we shouldn't be calling renderByCtor this much

export function render0(ctor: Ctor, f: (id: ID) => D): Render {
  return (cursor, sourceID) => renderByCtor(ctor, id => f(id) )(cursor, sourceID) }

export function render1(ctor: Ctor, field0: Field, f: (descend0: D, id: ID) => D, r0: Render = alwaysFail): Render {
  return (cursor, sourceID) =>
    renderByCtor(ctor, id =>
      render0(ctor, id => f(descend(cursor, id, field0.id, r0), id))(cursor, sourceID)
    )(cursor, sourceID) }

export function render2(ctor: Ctor, field0: Field, field1: Field, f: (descend0: D, descend1: D, id: ID) => D, r0: Render = alwaysFail, r1: Render = alwaysFail): Render {
  return (cursor, sourceID) =>
    renderByCtor(ctor, id =>
      render1(ctor, field0, (d0, id) => f(d0, descend(cursor, id, field1.id, r1), id), r0)(cursor, sourceID)
    )(cursor, sourceID) }

export function render3(ctor: Ctor, field0: Field, field1: Field, field2: Field, f: (descend0: D, descend1: D, descend2: D, id: ID) => D, r0: Render = alwaysFail, r1: Render = alwaysFail, r2: Render = alwaysFail): Render {
  return (cursor, sourceID) =>
    renderByCtor(ctor, id =>
      render2(ctor, field0, field1, (d0, d1, id) => f(d0, d1, descend(cursor, id, field2.id, r2), id), r0, r1)(cursor, sourceID)
    )(cursor, sourceID) }

export function renderByCtor(ctor: Ctor, f: (id: ID) => Maybe<D>): Render {
  return (cursor, sourceID) => bindMaybe(sourceID, sourceID => bindMaybe(_get(sourceID.id, ctorField.id), ctorID => ctorID === ctor.id ? f(sourceID.id) : nothing)) }