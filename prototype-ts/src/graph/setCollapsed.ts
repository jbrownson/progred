import { mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { SparseSpanningTree } from "./SparseSpanningTree"

export function setCollapsed(cursor: Cursor, collapsed: Maybe<boolean>) {
  function f(cursor: Cursor, sparseSpanningTree: SparseSpanningTree) {
    mapMaybe(cursor.parentCursor, parentCursor => maybe(parentCursor.sparseSpanningTree,
      () => f(parentCursor, new SparseSpanningTree(nothing, new Map([[cursor.label, sparseSpanningTree]]))),
      parentSparseSpanningTree => { parentSparseSpanningTree.map.set(cursor.label, sparseSpanningTree) } ))}
  maybe(cursor.sparseSpanningTree, () => f(cursor, new SparseSpanningTree(collapsed)), sparseSpanningTree => { sparseSpanningTree.collapsed = collapsed }) }

export function getCollapsed(cursor: Cursor): Maybe<boolean> { return mapMaybe(cursor.sparseSpanningTree, sparseSpanningTree => sparseSpanningTree.collapsed) }