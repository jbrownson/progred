import { Maybe } from "../../lib/Maybe"

export type MapID = string | number
export type NodeSetDelete = {node: Maybe<Node>, setDelete: Maybe<{set: (node: Node) => void, delete: () => void}>}

export interface Node {
  equals(node: Node): boolean
  get(label: Node): NodeSetDelete
  mapID: MapID }