import { Maybe, nothing, unsafeUnwrapMaybe } from "../lib/Maybe"
import { tuple } from "../lib/Tuple"
import { GUIDMap } from "./GUIDMap"
import { GUID, ID, nidFromNumber, sidFromString } from "./ID"

// TODO error checking
export function load({root, guidMap}: {root: Maybe<ID>, guidMap: { [s: string /*GUID*/]: {label: ID, to: ID}[]; }}): {root: Maybe<ID>, guidMap: GUIDMap} {
  return {root, guidMap: new GUIDMap(new Map(Object.keys(guidMap).map(guid => tuple(guid, new Map(guidMap[guid].map(({label, to}) =>
    tuple(unsafeUnwrapMaybe(jsonToID(label)), unsafeUnwrapMaybe(jsonToID(to)))))))))} }

function jsonToID(json: any): Maybe<ID> {
  return json.hasOwnProperty("guid") ? json["guid"] as GUID : json.hasOwnProperty("string") ? sidFromString(json.string) : json.hasOwnProperty("number") ? nidFromNumber(json.number) : nothing }