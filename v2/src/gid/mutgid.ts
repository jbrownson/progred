import { Map } from 'immutable'
import type { Id } from './id'
import { GuidId } from './id'
import type { Gid } from './gid'
import type { Maybe } from '../maybe'

export class MutGid {
  private data = Map<GuidId, Map<GuidId, Id>>()

  get(entity: Id, label: GuidId): Maybe<Id> {
    if (!(entity instanceof GuidId)) return undefined
    return this.data.get(entity)?.get(label)
  }

  edges(entity: Id): Maybe<Map<GuidId, Id>> {
    if (!(entity instanceof GuidId)) return undefined
    return this.data.get(entity)
  }

  set(entity: GuidId, label: GuidId, value: Id): void {
    const edges = this.data.get(entity) ?? Map<GuidId, Id>()
    this.data = this.data.set(entity, edges.set(label, value))
  }

  delete(entity: GuidId, label: GuidId): void {
    const edges = this.data.get(entity)
    if (edges) {
      const newEdges = edges.delete(label)
      this.data = newEdges.size === 0
        ? this.data.delete(entity)
        : this.data.set(entity, newEdges)
    }
  }

  has(entity: GuidId): boolean { return this.data.has(entity) }
  asGid(): Gid { return entity => this.edges(entity) }

  entities(): Iterable<GuidId> {
    return this.data.keys()
  }
}
