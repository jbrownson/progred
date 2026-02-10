import { bindMaybe, fromMaybe } from "../lib/Maybe"
import { GUID, guidFromID, ID } from "./ID"
import { IDMap } from "./IDMap"

export class MapIDMap implements IDMap {
  constructor(public map: Map<GUID, Map<ID, ID>> = new Map) {}
  edges(id: ID) { return fromMaybe<Map<ID, ID>>(bindMaybe(guidFromID(id), guid => this.map.get(guid)), () => new Map) }
  get(id: ID, label: ID) { return bindMaybe(bindMaybe(guidFromID(id), guid => this.map.get(guid)), edges => edges.get(label)) } }