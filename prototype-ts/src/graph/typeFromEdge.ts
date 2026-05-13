import { bindMaybe, Maybe } from "../lib/Maybe"
import { Field, Type } from "./graph"
import { EdgeRef } from "./model/EdgeRef"

export function typeFromEdge(edge: EdgeRef): Maybe<Type> {
  return bindMaybe(Field.fromID(edge.label), field => field.type)
}
