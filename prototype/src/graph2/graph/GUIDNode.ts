import { Maybe, nothing } from "../../lib/Maybe"
import { GUID } from "./GUIDMap"
import { Node, NodeSetDelete } from "./Node"
import { guidMap } from "./withGUIDMap"

export class GUIDNode implements Node {
  constructor(public guid: GUID) {}
  static fromNode(node: Node): Maybe<GUIDNode> { return node instanceof GUIDNode ? node : nothing }
  equals(node: Node): boolean { return node instanceof GUIDNode && node.guid === this.guid }
  get(label: Node): NodeSetDelete { return {node: guidMap().get(this.guid, label),
    setDelete: {set: node => guidMap().set(this.guid, label, node), delete: () => guidMap().delete(this.guid, label)}} }
  get mapID() { return `g${this.guid}`} }