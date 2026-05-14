import { bindMaybe, Maybe, nothing } from "../../lib/Maybe"
import { HasID } from "../graph"
import { stringFromID } from "../model/ID"
import { jsonFromJSON } from "./jsonFromJSON"

function parseJSON(string: string): Maybe<unknown> {
  try {
    return JSON.parse(string)
  } catch {
    return nothing }}

export function jsonFromString(hasID: HasID) { return bindMaybe(stringFromID(hasID.id), string => bindMaybe(parseJSON(string), jsonFromJSON)) }
