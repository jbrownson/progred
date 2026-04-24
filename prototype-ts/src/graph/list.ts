import { bindMaybe, map2Maybe,  Maybe } from "../lib/Maybe"
import { GUIDEmptyList, GUIDList, GUIDNonemptyList, HasID, List, matchList } from "./graph"
import { ID } from "./model/ID"

export function arrayFromList<A extends HasID>(list: List<A>): Maybe<A[]> {
  return matchList(list, nonemptyList => map2Maybe(nonemptyList.head, bindMaybe(nonemptyList.tail, arrayFromList), (head, tail) => [head, ...tail]), emptyList => []) }

export function listFromArray<A extends HasID>(as: A[], f: (id: ID) => Maybe<A>): GUIDList<A> {
  return as.length === 0 ? GUIDEmptyList.new() : GUIDNonemptyList.new(f).setHead(as[0]).setTail(listFromArray(as.slice(1), f)) }
