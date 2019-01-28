import { bindMaybe, fromMaybe, mapMaybe } from "../../lib/Maybe"
import { GUID, GUIDMap } from "./GUIDMap"
import { MapID, Node } from "./Node"

export class MapGUIDMap implements GUIDMap {
  private map = new Map<GUID, Map<MapID, {label: Node, node: Node}>>()
  static fromIterable(iterable: Iterable<{guid: GUID, edges: Iterable<{label: Node, node: Node}>}>) {
    let mapGUIDMap = new MapGUIDMap
    mapGUIDMap.map = new Map(Array.from(iterable).map(({guid, edges}) => [guid, new Map(Array.from(edges).map(({label, node}) =>
      [label.mapID, {label, node}] as [MapID, {label: Node, node: Node}] ))] as [GUID, Map<MapID, {label: Node, node: Node}>] ))
    return mapGUIDMap }
  isConsistent() {
    for (let edges of this.map.values())
      for (let [labelMapID, {label}] of edges)
        if (label.mapID !== labelMapID) return false
    return true }
  get(guid: GUID, label: Node) { return bindMaybe(this.map.get(guid), edges => mapMaybe(edges.get(label.mapID), ({node}) => node)) }
  edges(guid: GUID) { return mapMaybe(this.map.get(guid), edges => Array.from(edges.values())) }
  set(guid: GUID, label: Node, node: Node) {
    fromMaybe(this.map.get(guid), () => { let edges = new Map<MapID, {label: Node, node: Node}>(); this.map.set(guid, edges); return edges }).set(label.mapID, {label, node}) }
  sets(guid: GUID, newEdges: Iterable<{label: Node, node: Node}>) {
    let edges = fromMaybe(this.map.get(guid), () => { let edges = new Map<MapID, {label: Node, node: Node}>(); this.map.set(guid, edges); return edges })
    for (let {label, node} of newEdges) edges.set(label.mapID, {label, node}) }
  delete(guid: GUID, label: Node) {
    bindMaybe(this.map.get(guid), edges => {edges.delete(label.mapID); if (edges.size === 0) this.map.delete(guid)}) } }