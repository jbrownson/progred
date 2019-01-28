import { bindMaybe, map2Maybe,  Maybe } from "../lib/Maybe"
import { HasID, List, matchList } from "./graph"

export function arrayFromList<A extends HasID>(list: List<A>): Maybe<A[]> {
  return matchList(list, nonemptyList => map2Maybe(nonemptyList.head, bindMaybe(nonemptyList.tail, arrayFromList), (head, tail) => [head, ...tail]), emptyList => []) }