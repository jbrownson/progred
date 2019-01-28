import { mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { ctorCtor, ctorField, nameField } from "./graph"
import { ID, sidFromString, stringFromID } from "./ID"
import { IDMap } from "./IDMap"

// TOOD this isn't used, should it be?

export class StringsLibrary implements IDMap {
  edges(id: ID): Map<ID, ID> {
    return maybe(stringFromID(id), () => new Map, string => new Map([[ctorField.id, ctorCtor.id], [nameField.id, sidFromString(`"${string}"`)]])) }
  get(id: ID, label: ID): Maybe<ID> {
return mapMaybe(stringFromID(id), string => label === ctorField.id ? ctorCtor.id : label === nameField.id ? sidFromString(`"${string}"`) : nothing) }}