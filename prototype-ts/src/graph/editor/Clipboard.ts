import { bindMaybe, Maybe, maybeFromException } from "../../lib/Maybe"
import { ID, idFromJSON } from "../model/ID"
import { CopyResult, copyResultToJSON, idFromCopyJSON } from "./Copy"

export const clipboardFormat = "progred_custom_clipboard_format"
export const plainTextFormat = "text/plain"

export function copyIDFromClipboardText(text: Maybe<string>): Maybe<ID> {
  return bindMaybe(bindMaybe(text, text => maybeFromException(() => JSON.parse(text))), json =>
    bindMaybe(json.copy, idFromCopyJSON))}

export function idFromClipboardText(text: Maybe<string>): Maybe<ID> {
  return bindMaybe(bindMaybe(text, text => maybeFromException(() => JSON.parse(text))), json =>
    idFromJSON(json.id))}

export function clipboardStringForCopyResult(referenceID: ID, copyResult: CopyResult): string {
  return JSON.stringify({
    copy: copyResultToJSON(copyResult),
    id: referenceID }) }
