import { setDifference } from "../../lib/Array"
import { Maybe, maybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { D, Line, SupportsUnderselection } from "./D"
import { renderField } from "./defaultRender"
import { edges, SourceID, SourceType } from "../Environment"
import { ctorField, Field } from "../graph"
import { guidFromID } from "../model/ID"
import { pendingEdgeLabel } from "./pendingEdgeLabel"
import { selectedMissingLabels } from "./selectedMissingLabels"

export function renderOtherFields(cursor: Cursor, sourceID: Maybe<SourceID>, d: D, knownFields: Field[]): D {
  return maybe(sourceID, () => d, sourceID => {
    const allIds = maybe(edges(sourceID.id), () => [], ({edges}) => Array.from(edges).map(x => x[0]))
    const knownIds = [...knownFields, ctorField].map(field => field.id)
    const unknownIds = [
      ...setDifference(allIds, knownIds),
      ...selectedMissingLabels(cursor, sourceID.id, [...allIds, ...knownIds]) ]
    const guid = guidFromID(sourceID.id)
    const pendingEdgeLabelDs = maybe(guid, () => [], guid => pendingEdgeLabel(cursor, guid))
    const dWithOtherFields = unknownIds.length > 0 || pendingEdgeLabelDs.length > 0
      ? new Line(d, ...unknownIds.map(unknownId => renderField(cursor, sourceID.id, unknownId)), ...pendingEdgeLabelDs)
      : d
    return sourceID.source.source === SourceType.DocumentType && guid !== undefined
      ? new SupportsUnderselection(dWithOtherFields)
      : dWithOtherFields })}
