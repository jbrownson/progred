import { mapMaybe, maybe, Maybe } from "../lib/Maybe"
import { GUID, guidFromID, ID } from "./ID"

export class GUIDMap {
  constructor(public map: Map<GUID, Map<ID, ID>> = new Map) {}
  edges(guid: GUID): Maybe<Map<ID, ID>> { return this.map.get(guid) }
  get(guid: GUID, label: ID): Maybe<ID> {
    const edges = this.map.get(guid)
    if (edges !== undefined)
      edges.get(label)
    return undefined }
  set(guid: GUID, label: ID, to: ID) { maybe(this.map.get(guid), () => { this.map.set(guid, new Map([[label, to]])) }, edges => {edges.set(label, to)}) }
  delete(guid: GUID, label: ID) { mapMaybe(this.map.get(guid), edges => {edges.delete(label); if (edges.size === 0) this.map.delete(guid) }) } }

export function garbageCollectGUIDMap(guidMap: GUIDMap, root: ID): GUIDMap {
  let visited = new Set<GUID>()
  function _garbageCollectGUIDMap(id: ID) {
    mapMaybe(guidFromID(id), guid => {
      if (!visited.has(guid)) {
        visited.add(guid)
        mapMaybe(guidMap.map.get(guid), edges => Array.from(edges.values()).forEach(_garbageCollectGUIDMap)) }})}
  _garbageCollectGUIDMap(root)
  return new GUIDMap(new Map(Array.from(guidMap.map).filter(([k, v]) => visited.has(k)))) }