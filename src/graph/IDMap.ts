import { Maybe } from "../lib/Maybe"
import { ID } from "./ID"

export interface IDMap {
  edges(id: ID): Maybe<Map<ID, ID>>
  get(id: ID, label: ID): Maybe<ID> }