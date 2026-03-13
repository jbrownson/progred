import { Maybe } from "../lib/Maybe"
import { GUIDMap } from "./GUIDMap"
import { ID, matchID, numberFromNID } from "./ID"

export function save({root, guidMap}: {root: Maybe<ID>, guidMap: GUIDMap}): {root: Maybe<ID>, guidMap: JSON} {
  return {root, guidMap: Array.from(guidMap.map).reduce((json, [guid, edges]) => {
    json[guid] = Array.from(edges).map(([label, to]) => ({label: idToJSON(label), to: idToJSON(to)}))
    return json }, {} as any)} }

function idToJSON(id: ID) {
  return matchID<any>(id, guid => ({guid}), (sid, string) => ({string}), nid => ({number: numberFromNID(nid)})) }