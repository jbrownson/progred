import { mapMaybe, Maybe } from "../lib/Maybe"
import { HasSID, JSON } from "./graph"
import { sidFromString } from "./ID"
import { jsonToJSON } from "./jsonFromJSON"

export function stringFromJSON(json: JSON): Maybe<HasSID> {
  return mapMaybe(jsonToJSON(json), json => new HasSID(sidFromString(JSON.stringify(json, undefined, 2)))) }