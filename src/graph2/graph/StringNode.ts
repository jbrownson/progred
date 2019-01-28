import { Maybe, nothing } from "../../lib/Maybe"
import { Node } from "./Node"

export class StringNode implements Node {
  constructor(public string: string) {}
  static fromNode(node: Node): Maybe<StringNode> { return node instanceof StringNode ? node : nothing }
  equals(node: Node): boolean { return node instanceof StringNode && node.string === this.string }
  get() { return {node: nothing, setDelete: nothing} }
  get mapID() { return `s${this.string}` } }