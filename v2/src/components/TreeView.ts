import { GuidId, StringId, NumberId, matchId } from '../gid/id'
import type { Id } from '../gid/id'
import type { Gid } from '../gid/gid'
import type { Maybe } from '../maybe'
import type { SpanningTree } from '../spanningtree'
import { emptySpanningTree } from '../spanningtree'
import { pathsEqual, pathNode, rootPath, childPath, popPath } from '../path'
import type { Path } from '../path'
import {
  EdgeLabel, NodeIdenticon, CollapseToggle,
  SetTargetButton, UseAsLabelButton, NewNodeButton,
  NodeHeader, EmptyNode, LeafNode, EditableStringNode, EditableNumberNode,
  ChildrenList, ChildItem, GuidNodeWrapper, TreeViewContainer, InsertionPoint
} from './TreeRendering'

export type RootSlot = { id: GuidId, node: GuidId }

// TODO: insertAt index is not stable over list mutations
export type Selection =
  | { type: 'insertAt', index: number }
  | { type: 'path', path: Path }

export type TreeContext = {
  gid: Gid
  roots: RootSlot[]
  tree: SpanningTree
  selection: Maybe<Selection>
  setCollapsed: (path: Path, collapsed: boolean) => void
  select: (selection: Maybe<Selection>) => void
  insertRoot: (index: number, node: GuidId) => void
  setRootNode: (slotId: GuidId, node: GuidId) => void
  deleteRoot: (slotId: GuidId) => void
  setEdge: (parent: GuidId, label: GuidId, value: Id) => void
  deleteEdge: (parent: GuidId, label: GuidId) => void
  newNode: () => GuidId
}

function getRootNode(ctx: TreeContext, slot: GuidId): Maybe<GuidId> {
  return ctx.roots.find(r => r.id.equals(slot))?.node
}

function resolvePathNode(ctx: TreeContext, path: Path): Maybe<Id> {
  const root = getRootNode(ctx, path.rootSlot)
  return pathNode(ctx.gid, root, path)
}

function getSelectedPath(selection: Maybe<Selection>): Maybe<Path> {
  return selection?.type === 'path' ? selection.path : undefined
}

function getSelectedInsertIndex(selection: Maybe<Selection>): Maybe<number> {
  return selection?.type === 'insertAt' ? selection.index : undefined
}

function isSelectedPath(selection: Maybe<Selection>, path: Path): boolean {
  const selectedPath = getSelectedPath(selection)
  return selectedPath !== undefined && pathsEqual(path, selectedPath)
}

function selectPath(ctx: TreeContext, path: Path): void {
  ctx.select({ type: 'path', path })
}

function selectInsertAt(ctx: TreeContext, index: number): void {
  ctx.select({ type: 'insertAt', index })
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
  return EmptyNode(isSelectedPath(ctx.selection, path), () => selectPath(ctx, path))
}

function StringNode(ctx: TreeContext, node: StringId, path: Path): HTMLDivElement {
  const selected = isSelectedPath(ctx.selection, path)
  if (selected) {
    const popped = popPath(path)
    if (popped) {
      const parentNode = resolvePathNode(ctx, popped.parent)
      if (parentNode instanceof GuidId) {
        return EditableStringNode(
          node.value,
          value => ctx.setEdge(parentNode, popped.label, new StringId(value)),
          () => ctx.select(undefined)
        )
      }
    }
  }
  return LeafNode(node, selected, () => selectPath(ctx, path))
}

function NumberNode(ctx: TreeContext, node: NumberId, path: Path): HTMLDivElement {
  const selected = isSelectedPath(ctx.selection, path)
  if (selected) {
    const popped = popPath(path)
    if (popped) {
      const parentNode = resolvePathNode(ctx, popped.parent)
      if (parentNode instanceof GuidId) {
        return EditableNumberNode(
          node.value,
          value => ctx.setEdge(parentNode, popped.label, new NumberId(value)),
          () => ctx.select(undefined)
        )
      }
    }
  }
  return LeafNode(node, selected, () => selectPath(ctx, path))
}

function getPendingEdge(
  selection: Maybe<Path>,
  path: Path,
  edges: [GuidId, Id][]
): Maybe<GuidId> {
  if (!selection || selection.edges.length !== path.edges.length + 1) return undefined
  const popped = popPath(selection)
  if (!popped || !pathsEqual(popped.parent, path)) return undefined
  return edges.some(([edgeLabel]) => edgeLabel.equals(popped.label)) ? undefined : popped.label
}

function renderActionButtons(
  ctx: TreeContext,
  node: GuidId
): HTMLButtonElement[] {
  const selectedPath = getSelectedPath(ctx.selection)
  if (!selectedPath) return []

  const selectionNode = resolvePathNode(ctx, selectedPath)

  return [
    SetTargetButton(() => setAtPath(ctx, selectedPath, node)),
    ...(selectionNode instanceof GuidId
      ? [UseAsLabelButton(() => selectPath(ctx, childPath(selectedPath, node)))]
      : [])
  ]
}

function renderGuidNodeHeader(
  ctx: TreeContext,
  node: GuidId,
  path: Path,
  hasChildren: boolean,
  collapsed: boolean,
  selected: boolean
): HTMLDivElement {
  return NodeHeader(selected, () => selectPath(ctx, path),
    NodeIdenticon(node),
    hasChildren ? CollapseToggle(collapsed, () => ctx.setCollapsed(path, !collapsed)) : null,
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
  const pendingEdge = getPendingEdge(getSelectedPath(ctx.selection), path, edges)
  return ChildrenList(
    ...edges.map(([edgeLabel, childNode]) => {
      const edgePath = childPath(path, edgeLabel)
      const childSubtree = subtree.children.get(edgeLabel) ?? emptySpanningTree()
      return ChildItem(
        EdgeLabel(edgeLabel),
        TreeNode(ctx, childNode, edgePath, childSubtree, ancestors)
      )
    }),
    ...(pendingEdge ? [ChildItem(
      EdgeLabel(pendingEdge),
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
  const { gid } = ctx
  const edges = [...gid(node) ?? []]
  const hasChildren = edges.length > 0
  const collapsed = subtree.collapsed ?? ancestors.has(node)
  const childAncestors = new Set([...ancestors, node])

  const header = renderGuidNodeHeader(ctx, node, path, hasChildren, collapsed, isSelectedPath(ctx.selection, path))
  const children = collapsed ? null : renderChildren(ctx, path, subtree, childAncestors, edges)

  return GuidNodeWrapper(header, children)
}

export function setAtPath(ctx: TreeContext, path: Path, value: Maybe<GuidId>): void {
  const popped = popPath(path)
  if (popped) {
    const parentNode = resolvePathNode(ctx, popped.parent)
    if (parentNode instanceof GuidId) {
      if (value) {
        ctx.setEdge(parentNode, popped.label, value)
      } else {
        ctx.deleteEdge(parentNode, popped.label)
      }
    }
  } else {
    if (value) {
      ctx.setRootNode(path.rootSlot, value)
    } else {
      ctx.deleteRoot(path.rootSlot)
    }
  }
}

function RootSlotView(ctx: TreeContext, slot: RootSlot): HTMLDivElement {
  const path = rootPath(slot.id)
  return TreeNode(ctx, slot.node, path, ctx.tree, new Set())
}

function RootInsertionPoint(ctx: TreeContext, index: number): HTMLDivElement {
  const selectedIndex = getSelectedInsertIndex(ctx.selection)
  return InsertionPoint(selectedIndex === index, index === 0, () => selectInsertAt(ctx, index))
}

export function TreeView(ctx: TreeContext): HTMLDivElement {
  const { roots, selection } = ctx
  const selectedPath = getSelectedPath(selection)
  const selectedIndex = getSelectedInsertIndex(selection)

  if (roots.length === 0) {
    return TreeViewContainer(
      () => ctx.select(undefined),
      NewNodeButton(() => ctx.insertRoot(0, ctx.newNode()))
    )
  }

  const elements: (HTMLElement | null)[] = []
  for (let i = 0; i <= roots.length; i++) {
    elements.push(RootInsertionPoint(ctx, i))
    if (i < roots.length) {
      elements.push(RootSlotView(ctx, roots[i]))
    }
  }

  return TreeViewContainer(
    () => ctx.select(undefined),
    ...elements,
    selectedIndex !== undefined
      ? NewNodeButton(() => ctx.insertRoot(selectedIndex, ctx.newNode()))
      : null,
    selectedPath
      ? NewNodeButton(() => setAtPath(ctx, selectedPath, ctx.newNode()))
      : null
  )
}
