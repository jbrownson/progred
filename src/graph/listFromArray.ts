import { Maybe } from "../lib/Maybe"
import { GUIDEmptyList, GUIDList, GUIDNonemptyList, HasID } from "./graph"
import { ID } from "./ID"

export function listFromArray<A extends HasID>(as: A[], f: (id: ID) => Maybe<A>): GUIDList<A> {
  return as.length === 0 ? GUIDEmptyList.new() : GUIDNonemptyList.new(f).setHead(as[0]).setTail(listFromArray(as.slice(1), f)) }