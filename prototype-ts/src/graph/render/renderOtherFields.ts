import { setDifference } from "../../lib/Array"
import { Maybe, maybe } from "../../lib/Maybe"
import type { D } from "./DContext"
import { line } from "./DLayout"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"
import { edges, SourceID, SourceType } from "../Environment"
import { ctorField, Field } from "../graph"
import { Edge } from "../model/Edge"
import { emptyCyclePath, type CyclePath } from "./CyclePath"
import { edgeContextForEdge } from "../editor/edgeContext"

export function renderOtherFields(edge: Edge, sourceID: Maybe<SourceID>, d: D, knownFields: Field[], cyclePath: CyclePath = emptyCyclePath()): D {
  return maybe(sourceID, () => d, sourceID => {
    const sourceEdges = edges(sourceID.id)
    const allIds = maybe(sourceEdges, () => [], ({edges}) => Array.from(edges).map(x => x[0]))
    const writable = maybe(sourceEdges, () => sourceID.source.source === SourceType.DocumentType, ({source}) => source.source === SourceType.DocumentType)
    const knownIds = [...knownFields, ctorField].map(field => field.id)
    const unknownIds = setDifference(allIds, knownIds)
    const dWithOtherFields = unknownIds.length > 0
      ? line(d, ...unknownIds.map(unknownId => {
        let edgeContext = edgeContextForEdge({parent: sourceID.id, label: unknownId})
        return renderField(sourceID.id, unknownId, writable ? edgeContext : {...edgeContext, commit: undefined}, cyclePath) }))
      : d
    return renderDocumentGuidEditor(edge, sourceID, dWithOtherFields) })}
