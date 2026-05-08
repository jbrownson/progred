import { Maybe } from "../../lib/Maybe"
import { GUIDMap } from "./GUIDMap"
import { ID, matchID, numberFromNID } from "./ID"

export type JSONID = {guid: string} | {string: string} | {number: number}
export type SerializedGraph = {root: Maybe<ID>, guidMap: {[guid: string]: {label: JSONID, to: JSONID}[]}}

export function save({root, guidMap}: {root: Maybe<ID>, guidMap: GUIDMap}): SerializedGraph {
  return {root, guidMap: Array.from(guidMap.map).reduce((json, [guid, edges]) => {
    json[guid] = Array.from(edges).map(([label, to]) => ({label: idToJSON(label), to: idToJSON(to)}))
    return json }, {} as SerializedGraph["guidMap"])} }

function idToJSON(id: ID): JSONID {
  return matchID<JSONID>(id, guid => ({guid}), (sid, string) => ({string}), nid => ({number: numberFromNID(nid)})) }
