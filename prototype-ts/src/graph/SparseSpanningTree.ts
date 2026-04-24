import { ID } from "./model/ID"

export class SparseSpanningTree {
  constructor(public collapsed?: boolean, public map = new Map<ID, SparseSpanningTree>()) {} }
