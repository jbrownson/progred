import { ID } from "./ID"

export class SparseSpanningTree {
  constructor(public collapsed?: boolean, public map = new Map<ID, SparseSpanningTree>()) {} }