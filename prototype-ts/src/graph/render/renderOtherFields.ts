import { setDifference } from "../../lib/Array"
import { Maybe, maybe } from "../../lib/Maybe"
import type { D } from "./DContext"
import { line } from "./DLayout"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"
import { edges, SourceID } from "../Environment"
import { ctorField, Field } from "../graph"
import { Edge } from "../model/Edge"
import { emptyCyclePath, type CyclePath } from "./CyclePath"

export function renderOtherFields(edge: Edge, sourceID: Maybe<SourceID>, d: D, knownFields: Field[], cyclePath: CyclePath = emptyCyclePath()): D {
  return maybe(sourceID, () => d, sourceID => {
    const allIds = maybe(edges(sourceID.id), () => [], ({edges}) => Array.from(edges).map(x => x[0]))
    const knownIds = [...knownFields, ctorField].map(field => field.id)
    const unknownIds = setDifference(allIds, knownIds)
    const dWithOtherFields = unknownIds.length > 0
      ? line(d, ...unknownIds.map(unknownId => renderField(sourceID.id, unknownId, undefined, cyclePath)))
      : d
    return renderDocumentGuidEditor(edge, sourceID, dWithOtherFields) })}
