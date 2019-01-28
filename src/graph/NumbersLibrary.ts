import { mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { ctorCtor, ctorField, nameField } from "./graph"
import { ID, numberFromID, sidFromString } from "./ID"
import { IDMap } from "./IDMap"

// TOOD this isn't used, should it be?

export class NumbersLibrary implements IDMap {
  edges(id: ID): Map<ID, ID> {
    return maybe(numberFromID(id), () => new Map, number => new Map([[ctorField.id, ctorCtor.id], [nameField.id, sidFromString(`${number}`)]])) }
  get(id: ID, label: ID): Maybe<ID> {
return mapMaybe(numberFromID(id), number => label === ctorField.id ? ctorCtor.id : label === nameField.id ? sidFromString(`${number}`) : nothing) }}