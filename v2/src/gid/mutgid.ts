import { Map } from 'immutable'
import type { Id } from './id'
import { GuidId, idFromJSON } from './id'
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

  toJSON(): Record<string, Record<string, object>> {
    const result: Record<string, Record<string, object>> = {}
    for (const [entity, edges] of this.data) {
      const edgeObj: Record<string, object> = {}
      for (const [label, value] of edges) {
        edgeObj[label.guid] = value.toJSON()
      }
      result[entity.guid] = edgeObj
    }
    return result
  }

  static fromJSON(json: Record<string, Record<string, unknown>>): MutGid {
    const gid = new MutGid()
    for (const [entityGuid, edges] of Object.entries(json)) {
      const entity = new GuidId(entityGuid)
      for (const [labelGuid, value] of Object.entries(edges)) {
        const label = new GuidId(labelGuid)
        const id = idFromJSON(value)
        if (id) {
          gid.set(entity, label, id)
        }
      }
    }
    return gid
  }
}
