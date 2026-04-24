import { assert } from "../../lib/assert"
import { generateGUID as generateRawGUID } from "../../lib/generateGUID"
import { Maybe, nothing } from "../../lib/Maybe"

export type GUID = string
export type SID = string
export type NID = number
export type ID = GUID | SID | NID

export function matchID<A>(id: ID, guidF: (guid: GUID) => A, sidF: (sid: SID, string: string) => A, nidF: (nid: NID) => A): A {
  return typeof id === "number" ? nidF(id) : id.startsWith("sid:") ? sidF(id, stringFromSID(id)) : guidF(id) }

export function generateGUID(): GUID {
  return generateRawGUID() }

export function sidFromID(id: ID): Maybe<SID> { return typeof id === "string" && id.startsWith("sid:") ? id : nothing }
export function nidFromID(id: ID): Maybe<NID> { return typeof id === "number" ? id : nothing }
export function sidFromString(string: string) { return `sid:${string}` }
export function stringFromSID(sid: SID) { assert(sid.startsWith('sid:')); return sid.slice(4) }
export function stringFromID(id: ID): Maybe<string> { return typeof id === 'string' && id.startsWith('sid:') ? stringFromSID(id) : nothing }
export function nidFromNumber(number: number) { return number }
export function numberFromNID(nid: NID) { return nid }
export function numberFromID(id: ID) { return typeof id === "number" ? id : nothing }
export function guidFromID(id: ID): Maybe<GUID> { return typeof id === 'string' && !id.startsWith('sid:') ? id : nothing } // TODO actually check
