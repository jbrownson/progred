import type { Id } from './id'
import { GuidId } from './id'
import { Gid } from './gid'
import { Maybe, map } from '../maybe'

export class MutGid {
  private data: Map<string, Map<string, Id>> = new Map()

  get(node: Id, label: Id): Maybe<Id> {
    if (!(node instanceof GuidId)) return undefined
    if (!(label instanceof GuidId)) return undefined
    return this.data.get(node.guid)?.get(label.guid)
  }

  set(node: GuidId, label: GuidId, value: Id): void {
    let edges = this.data.get(node.guid)
    if (!edges) {
      edges = new Map()
      this.data.set(node.guid, edges)
    }
    edges.set(label.guid, value)
  }

  delete(node: GuidId, label: GuidId): void {
    const edges = this.data.get(node.guid)
    if (edges) {
      edges.delete(label.guid)
      if (edges.size === 0) {
        this.data.delete(node.guid)
      }
    }
  }

  edges(node: GuidId): Maybe<Map<string, Id>> { return this.data.get(node.guid) }
  has(node: GuidId): boolean { return this.data.has(node.guid) }
  asGid(): Gid { return (node, label) => this.get(node, label) }

  nodes(): Iterable<[GuidId, [GuidId, Id][]]> {
    return map(this.data, ([guid, edges]) =>
      [new GuidId(guid), [...edges].map(([labelGuid, value]) => [new GuidId(labelGuid), value])]
    )
  }
}
