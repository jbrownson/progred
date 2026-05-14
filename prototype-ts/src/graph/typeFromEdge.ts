import { bindMaybe, Maybe } from "../lib/Maybe"
import { Field, Type } from "./graph"
import { Edge } from "./model/Edge"

export function typeFromEdge(edge: Edge): Maybe<Type> {
  return bindMaybe(Field.fromID(edge.label), field => field.type)
}
