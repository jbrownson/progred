import { Maybe } from "../../lib/Maybe"
import { Node } from "./Node"

export type GUID = string

export interface ROGUIDMap {
  get(guid: GUID, label: Node): Maybe<Node>
  edges(guid: GUID): Maybe<Iterable<{label: Node, node: Node}>> }

export interface GUIDMap extends ROGUIDMap {
  set(guid: GUID, label: Node, node: Node): void
  sets(guid: GUID, newEdges: Iterable<{label: Node, node: Node}>): void
  delete(guid: GUID, label: Node): void }