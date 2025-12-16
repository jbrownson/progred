import type { Id } from './id'
import { GuidId, idFromJSON } from './id'
import { Maybe, map, traverse } from '../maybe'
import { MutGid } from './mutgid'

type SerializedGid = {
  nodes: Record<string, Record<string, unknown>>
}

export function serialize(gid: MutGid): string {
  const nodes = Object.fromEntries(
    map(gid.nodes(), ([node, edges]) => [
      node.guid,
      Object.fromEntries(edges.map(([label, value]) => [label.guid, value.toJSON()]))
    ])
  )
  return JSON.stringify({ nodes }, null, 2)
}

type ParsedEdge = { node: GuidId, label: GuidId, id: Id }

export function deserialize(json: string): Maybe<MutGid> {
  const data: SerializedGid = JSON.parse(json)

  const parsed = traverse(
    Object.entries(data.nodes),
    ([nodeGuid, edges]) => traverse(
      Object.entries(edges),
      ([labelGuid, value]): Maybe<ParsedEdge> => {
        const id = idFromJSON(value)
        return id && { node: new GuidId(nodeGuid), label: new GuidId(labelGuid), id }
      }
    )
  )

  if (parsed === undefined) return undefined

  const gid = new MutGid()
  parsed.flat().forEach(({ node, label, id }) => gid.set(node, label, id))
  return gid
}
