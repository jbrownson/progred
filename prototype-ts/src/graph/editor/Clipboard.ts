import { bindMaybe, fromMaybe, Maybe, nothing } from "../../lib/Maybe"
import { ID, matchID, nidFromNumber, sidFromString } from "../model/ID"
import { idFromStructure, structureForCursor } from "../structureForID"
import type { Cursor } from "../cursor/Cursor"
import type { Descend } from "../render/D"

export const clipboardFormat = "progred_custom_clipboard_format"
export const plainTextFormat = "text/plain"

export function structureIDFromClipboardText(text: Maybe<string>): Maybe<ID> {
  try {
    let json = JSON.parse(fromMaybe(text, () => ""))
    return idFromStructure(JSON.parse(json.structure)) }
  catch(e) {}
  return nothing }

export function idFromClipboardText(text: Maybe<string>): Maybe<ID> {
  try {
    let json = JSON.parse(JSON.parse(fromMaybe(text, () => "")).id)
    return bindMaybe(json.string, jsonString => {
      if (typeof jsonString !== "string") return nothing
      switch (json.type) {
        case "guid": return jsonString
        case "number": let number = Number(jsonString); return !Number.isNaN(number) ? nidFromNumber(number) : nothing
        case "string": return sidFromString(jsonString) }})}
  catch(e) {}
  return nothing }

export function clipboardStringForID(id: ID): string {
  return JSON.stringify(matchID<{type: string, string: string}>(id,
    guid => ({type: "guid", string: guid}),
    (sid, s) => ({type: "string", string: s}),
    nid => ({type: "number", string: String(nid)}) ))}

export function clipboardStringForStructure(cursor: Cursor, rootDescend: Descend, viewsDescend: Maybe<Descend>): string {
  return JSON.stringify(structureForCursor(cursor, rootDescend, viewsDescend)) }
