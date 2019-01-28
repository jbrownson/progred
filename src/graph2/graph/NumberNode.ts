import { Maybe, nothing } from "../../lib/Maybe"
import { Node } from "./Node"

export class NumberNode implements Node {
  constructor(public number: number) {}
  static fromNode(node: Node): Maybe<NumberNode> { return node instanceof NumberNode ? node : nothing }
  equals(node: Node): boolean { return node instanceof NumberNode && node.number === this.number }
  get() { return {node: nothing, setDelete: nothing} }
  get mapID() { return this.number } }