import { ID, GuidID } from './id'
import { Graph } from './graph'
import { Maybe } from '../maybe'

export class Store {
  private data: Map<string, Map<string, ID>> = new Map()

  get(node: ID, label: ID): Maybe<ID> {
    if (!(node instanceof GuidID)) return undefined
    if (!(label instanceof GuidID)) return undefined
    return this.data.get(node.guid)?.get(label.guid)
  }

  set(node: GuidID, label: GuidID, value: ID): void {
    let edges = this.data.get(node.guid)
    if (!edges) {
      edges = new Map()
      this.data.set(node.guid, edges)
    }
    edges.set(label.guid, value)
  }

  delete(node: GuidID, label: GuidID): void {
    const edges = this.data.get(node.guid)
    if (edges) {
      edges.delete(label.guid)
      if (edges.size === 0) {
        this.data.delete(node.guid)
      }
    }
  }

  edges(node: GuidID): Maybe<Map<string, ID>> { return this.data.get(node.guid) }
  has(node: GuidID): boolean { return this.data.has(node.guid) }
  asGraph(): Graph { return (node, label) => this.get(node, label) }

  *nodes(): IterableIterator<[GuidID, [GuidID, ID][]]> {
    for (const [guid, edges] of this.data) {
      yield [new GuidID(guid), [...edges].map(([labelGuid, value]) => [new GuidID(labelGuid), value])]
    }
  }
}
