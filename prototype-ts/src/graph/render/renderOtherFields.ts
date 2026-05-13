import { setDifference } from "../../lib/Array"
import { Maybe, maybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { D, Line } from "./D"
import { renderDocumentGuidEditor, renderField } from "./defaultRender"
import { edges, SourceID } from "../Environment"
import { ctorField, Field } from "../graph"

export function renderOtherFields(cursor: Cursor, sourceID: Maybe<SourceID>, d: D, knownFields: Field[]): D {
  return maybe(sourceID, () => d, sourceID => {
    const allIds = maybe(edges(sourceID.id), () => [], ({edges}) => Array.from(edges).map(x => x[0]))
    const knownIds = [...knownFields, ctorField].map(field => field.id)
    const unknownIds = setDifference(allIds, knownIds)
    const dWithOtherFields = unknownIds.length > 0
      ? new Line(d, ...unknownIds.map(unknownId => renderField(cursor, sourceID.id, unknownId)))
      : d
    return renderDocumentGuidEditor(cursor, sourceID, dWithOtherFields) })}
