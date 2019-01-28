import { setDifference } from "../lib/Array"
import { Maybe, maybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { D, Line } from "./D"
import { renderField } from "./defaultRender"
import { edges, SourceID } from "./Environment"
import { ctorField, Field } from "./graph"

export function renderOtherFields(cursor: Cursor, sourceID: Maybe<SourceID>, d: D, knownFields: Field[]): D {
  return maybe(sourceID, () => d, sourceID => {
    return maybe(edges(sourceID.id), () => d, ({edges}) => {
      const allIds = Array.from(edges).map(x => x[0])
      knownFields.push(ctorField)
      const knownIds = knownFields.map(field => field.id)
      const unknownIds = setDifference(allIds, knownIds)
      return unknownIds.length > 0
        ? new Line(d, ...unknownIds.map(unknownId => renderField(cursor, sourceID.id, unknownId)))
        : d })})}