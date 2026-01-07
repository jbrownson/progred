import { GuidId, StringId, NumberId, matchId } from '../gid/id'
import type { Id } from '../gid/id'
import type { Gid } from '../gid/gid'
import type { Maybe } from '../maybe'
import type { SpanningTree } from '../spanningtree'
import { emptySpanningTree } from '../spanningtree'
import { pathsEqual, pathNode, emptyPath, childPath, popPath } from '../path'
import type { Path } from '../path'
import {
  EdgeLabel, NodeIdenticon, CollapseToggle,
  SetTargetButton, UseAsLabelButton, NewNodeButton,
  NodeHeader, EmptyNode, LeafNode, ChildrenList, ChildItem,
  GuidNodeWrapper, TreeViewContainer
} from './TreeRendering'

export type TreeContext = {
  gid: Gid
  root: Maybe<GuidId>
  tree: SpanningTree
  selection: Maybe<Path>
  setCollapsed: (path: Path, collapsed: boolean) => void
  select: (path: Maybe<Path>) => void
  setRoot: (value: GuidId) => void
  setEdge: (parent: GuidId, label: GuidId, value: GuidId) => void
  clearRoot: () => void
  deleteEdge: (parent: GuidId, label: GuidId) => void
  newNode: () => GuidId
}

function isSelected(selection: Maybe<Path>, path: Path): boolean {
  return selection !== undefined && pathsEqual(path, selection)
}

function TreeNode(
  ctx: TreeContext,
  node: Maybe<Id>,
  path: Path,
  subtree: SpanningTree,
  ancestors: Set<Id>
): HTMLDivElement {
  return node
    ? matchId(node, {
        guid: id => GuidNode(ctx, id, path, subtree, ancestors),
        string: id => StringNode(ctx, id, path),
        number: id => NumberNode(ctx, id, path)
      })
    : PlaceholderNode(ctx, path)
}

function PlaceholderNode(ctx: TreeContext, path: Path): HTMLDivElement {
  return EmptyNode(isSelected(ctx.selection, path), () => ctx.select(path))
}

function StringNode(ctx: TreeContext, node: StringId, path: Path): HTMLDivElement {
  return LeafNode(node, isSelected(ctx.selection, path), () => ctx.select(path))
}

function NumberNode(ctx: TreeContext, node: NumberId, path: Path): HTMLDivElement {
  return LeafNode(node, isSelected(ctx.selection, path), () => ctx.select(path))
}

function getPendingEdge(
  selection: Maybe<Path>,
  path: Path,
  edges: [GuidId, Id][]
): Maybe<GuidId> {
  if (!selection || selection.length !== path.length + 1) return undefined
  const popped = popPath(selection)
  if (!popped || !pathsEqual(popped.parent, path)) return undefined
  return edges.some(([edgeLabel]) => edgeLabel.equals(popped.label)) ? undefined : popped.label
}

function renderActionButtons(
  ctx: TreeContext,
  node: GuidId
): HTMLButtonElement[] {
  const { gid, root, selection } = ctx
  if (!selection) return []

  const selectionNode = pathNode(gid, root, selection)

  return [
    SetTargetButton(() => setAtPath(ctx, selection, node)),
    ...(selectionNode instanceof GuidId
      ? [UseAsLabelButton(() => ctx.select(childPath(selection, node)))]
      : [])
  ]
}

function renderGuidNodeHeader(
  ctx: TreeContext,
  node: GuidId,
  path: Path,
  edges: [GuidId, Id][],
  collapsed: boolean,
  selected: boolean
): HTMLDivElement {
  return NodeHeader(selected, () => ctx.select(path),
    edges.length > 0 ? CollapseToggle(collapsed, () => ctx.setCollapsed(path, !collapsed)) : null,
    NodeIdenticon(node),
    ...renderActionButtons(ctx, node)
  )
}

function renderChildren(
  ctx: TreeContext,
  path: Path,
  subtree: SpanningTree,
  ancestors: Set<Id>,
  edges: [GuidId, Id][]
): HTMLUListElement {
  const pendingEdge = getPendingEdge(ctx.selection, path, edges)
  return ChildrenList(
    ...edges.map(([edgeLabel, childNode]) => {
      const edgePath = childPath(path, edgeLabel)
      const childSubtree = subtree.children.get(edgeLabel) ?? emptySpanningTree()
      return ChildItem(
        ...EdgeLabel(edgeLabel),
        TreeNode(ctx, childNode, edgePath, childSubtree, ancestors)
      )
    }),
    ...(pendingEdge ? [ChildItem(
      ...EdgeLabel(pendingEdge),
      TreeNode(ctx, undefined, childPath(path, pendingEdge), emptySpanningTree(), ancestors)
    )] : [])
  )
}

function GuidNode(
  ctx: TreeContext,
  node: GuidId,
  path: Path,
  subtree: SpanningTree,
  ancestors: Set<Id>
): HTMLDivElement {
  const { gid, selection } = ctx
  const edges = [...gid(node) ?? []]
  const collapsed = subtree.collapsed ?? ancestors.has(node)
  const childAncestors = new Set([...ancestors, node])

  const header = renderGuidNodeHeader(ctx, node, path, edges, collapsed, isSelected(selection, path))
  const children = collapsed ? null : renderChildren(ctx, path, subtree, childAncestors, edges)

  return GuidNodeWrapper(header, children)
}

export function setAtPath(ctx: TreeContext, path: Path, value: Maybe<GuidId>): void {
  const popped = popPath(path)
  if (popped) {
    const parentNode = pathNode(ctx.gid, ctx.root, popped.parent)
    if (parentNode instanceof GuidId) {
      if (value) {
        ctx.setEdge(parentNode, popped.label, value)
      } else {
        ctx.deleteEdge(parentNode, popped.label)
      }
    }
  } else {
    if (value) {
      ctx.setRoot(value)
    } else {
      ctx.clearRoot()
    }
  }
}

export function TreeView(ctx: TreeContext): HTMLDivElement {
  const { root, tree, selection } = ctx
  return TreeViewContainer(
    () => ctx.select(undefined),
    TreeNode(ctx, root, emptyPath, tree, new Set()),
    selection
      ? NewNodeButton(() => setAtPath(ctx, selection, ctx.newNode()))
      : null
  )
}
