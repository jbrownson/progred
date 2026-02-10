import { bindMaybe } from "../lib/Maybe"
import { HasID } from "./graph"
import { stringFromID } from "./ID"
import { jsonFromJSON } from "./jsonFromJSON"

export function jsonFromString(hasID: HasID) { return bindMaybe(stringFromID(hasID.id), string => jsonFromJSON(JSON.parse(string))) }