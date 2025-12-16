import { GuidID, idFromJSON } from './id'
import { Maybe } from '../maybe'
import { Store } from './store'

type SerializedGraph = {
  nodes: Record<string, Record<string, unknown>>
}

export function serialize(store: Store): string {
  const nodes: SerializedGraph['nodes'] = {}

  for (const [node, edges] of store.nodes()) {
    const edgeObj: Record<string, unknown> = {}
    for (const [label, value] of edges) {
      edgeObj[label.guid] = value.toJSON()
    }
    nodes[node.guid] = edgeObj
  }

  return JSON.stringify({ nodes }, null, 2)
}

export function deserialize(json: string): Maybe<Store> {
  const data: SerializedGraph = JSON.parse(json)
  const store = new Store()

  for (const [nodeGuid, edges] of Object.entries(data.nodes)) {
    const node = new GuidID(nodeGuid)
    for (const [labelGuid, value] of Object.entries(edges)) {
      const id = idFromJSON(value)
      if (id === undefined) return undefined
      store.set(node, new GuidID(labelGuid), id)
    }
  }

  return store
}
