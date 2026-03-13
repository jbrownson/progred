import { altMaybe, Maybe } from "../../lib/Maybe"
import { GUID, GUIDMap, ROGUIDMap } from "./GUIDMap"
import { Node } from "./Node"

export class ComposedGUIDMap implements GUIDMap {
  constructor(public guidMap: GUIDMap, public roGUIDMap: ROGUIDMap) {}
  get(guid: GUID, label: Node): Maybe<Node> { return altMaybe(this.guidMap.get(guid, label), () => this.roGUIDMap.get(guid, label)) }
  edges(guid: GUID): Maybe<Iterable<{label: Node, node: Node}>> { return altMaybe(this.guidMap.edges(guid), () => this.roGUIDMap.edges(guid)) }
  set(guid: GUID, label: Node, node: Node): void { this.guidMap.set(guid, label, node) }
  sets(guid: GUID, newEdges: Iterable<{label: Node, node: Node}>): void { this.guidMap.sets(guid, newEdges) }
  delete(guid: GUID, label: Node): void { this.guidMap.delete(guid, label) } }