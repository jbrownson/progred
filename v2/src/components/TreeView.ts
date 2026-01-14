import { el } from '../dom'
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
  NodeHeader, EmptyNode, EditablePlaceholder, LeafNode, EditableStringNode, EditableNumberNode,
  ChildrenList, ChildItem, GuidNodeWrapper, TreeViewContainer, InsertionPoint, LabelSlot
} from './TreeRendering'

export type RootSlot = { id: GuidId, node: GuidId }

// TODO: insertAt index is not stable over list mutations
export type Selection =
  | { type: 'insertAt', index: number }
  | { type: 'path', path: Path }
  | { type: 'nameLabel' }
  | { type: 'isaLabel' }

export type TreeContext = {
  gid: Gid
  roots: RootSlot[]
  tree: SpanningTree
  selection: Maybe<Selection>
  nameLabel: Maybe<GuidId>
  isaLabel: Maybe<GuidId>
  setCollapsed: (path: Path, collapsed: boolean) => void
  select: (selection: Maybe<Selection>) => void
  insertRoot: (index: number, node: GuidId) => void
  setRootNode: (slotId: GuidId, node: GuidId) => void
  deleteRoot: (slotId: GuidId) => void
  setEdge: (parent: GuidId, label: GuidId, value: Id) => void
  deleteEdge: (parent: GuidId, label: GuidId) => void
  setNameLabel: (label: Maybe<GuidId>) => void
  setIsaLabel: (label: Maybe<GuidId>) => void
  newNode: () => GuidId
}

function getRootNode(ctx: TreeContext, slot: GuidId): Maybe<GuidId> {
  return ctx.roots.find(r => r.id.equals(slot))?.node
}

function resolvePathNode(ctx: TreeContext, path: Path): Maybe<Id> {
  const root = getRootNode(ctx, path.rootSlot)
  return pathNode(ctx.gid, root, path)
}

function matchSelection<T>(
  selection: Maybe<Selection>,
  cases: { path: (p: Path) => T, insertAt: (i: number) => T, nameLabel: () => T, isaLabel: () => T, none: () => T }
): T {
  if (!selection) return cases.none()
  switch (selection.type) {
    case 'path': return cases.path(selection.path)
    case 'insertAt': return cases.insertAt(selection.index)
    case 'nameLabel': return cases.nameLabel()
    case 'isaLabel': return cases.isaLabel()
  }
}

function getSelectedPath(selection: Maybe<Selection>): Maybe<Path> {
  return matchSelection(selection, { path: p => p, insertAt: () => undefined, nameLabel: () => undefined, isaLabel: () => undefined, none: () => undefined })
}

function getSelectedInsertIndex(selection: Maybe<Selection>): Maybe<number> {
  return matchSelection(selection, { path: () => undefined, insertAt: i => i, nameLabel: () => undefined, isaLabel: () => undefined, none: () => undefined })
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
  const selected = isSelectedPath(ctx.selection, path)
  if (selected) {
    const popped = popPath(path)
    if (popped) {
      const parentNode = resolvePathNode(ctx, popped.parent)
      if (parentNode instanceof GuidId) {
        return EditablePlaceholder(
          id => ctx.setEdge(parentNode, popped.label, id),
          () => ctx.select(undefined)
        )
      }
    }
  }
  return EmptyNode(selected, () => selectPath(ctx, path))
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
  ctx: TreeContext,
  selection: Maybe<Path>,
  path: Path,
  edges: [GuidId, Id][]
): Maybe<GuidId> {
  if (!selection || selection.edges.length !== path.edges.length + 1) return undefined
  const popped = popPath(selection)
  if (!popped || !pathsEqual(popped.parent, path)) return undefined
  // Don't create pending edge for projected labels (name/isa) - they're shown in header
  if (isProjectedLabel(ctx, popped.label)) return undefined
  return edges.some(([edgeLabel]) => edgeLabel.equals(popped.label)) ? undefined : popped.label
}

function renderActionButtons(
  ctx: TreeContext,
  node: GuidId
): HTMLButtonElement[] {
  const { selection } = ctx

  if (selection?.type === 'nameLabel') {
    return [SetTargetButton(() => ctx.setNameLabel(node))]
  }
  if (selection?.type === 'isaLabel') {
    return [SetTargetButton(() => ctx.setIsaLabel(node))]
  }

  const selectedPath = getSelectedPath(selection)
  if (!selectedPath) return []

  const selectionNode = resolvePathNode(ctx, selectedPath)

  return [
    SetTargetButton(() => setAtPath(ctx, selectedPath, node)),
    ...(selectionNode instanceof GuidId
      ? [UseAsLabelButton(() => selectPath(ctx, childPath(selectedPath, node)))]
      : [])
  ]
}

function getNodeName(ctx: TreeContext, node: GuidId): Maybe<string> {
  if (!ctx.nameLabel) return undefined
  const edges = ctx.gid(node)
  if (!edges) return undefined
  const nameValue = edges.get(ctx.nameLabel)
  return nameValue instanceof StringId ? nameValue.value : undefined
}

function getNodeIsaName(ctx: TreeContext, node: GuidId): Maybe<string> {
  if (!ctx.isaLabel) return undefined
  const edges = ctx.gid(node)
  if (!edges) return undefined
  const isaNode = edges.get(ctx.isaLabel)
  if (!(isaNode instanceof GuidId)) return undefined
  return getNodeName(ctx, isaNode)
}

function renderIsaProjection(ctx: TreeContext, node: GuidId): HTMLElement | null {
  const isaName = getNodeIsaName(ctx, node)
  if (!isaName) return null
  return el('span', { style: { color: '#666', fontStyle: 'italic' } }, isaName)
}

function renderNameProjection(
  ctx: TreeContext,
  node: GuidId,
  path: Path
): HTMLElement | null {
  if (!ctx.nameLabel) return null
  const namePath = childPath(path, ctx.nameLabel)
  const nameSelected = isSelectedPath(ctx.selection, namePath)
  const name = getNodeName(ctx, node)
  if (!name && !nameSelected) return null

  if (nameSelected) {
    return EditableStringNode(
      name ?? '',
      value => ctx.setEdge(node, ctx.nameLabel!, new StringId(value)),
      () => {} // Blur just saves - click target handles selection
    )
  }

  return el('span', {
    style: { color: '#2a9d2a', cursor: 'pointer' },
    onMouseDown: (e: Event) => {
      if (document.activeElement instanceof HTMLInputElement) document.activeElement.blur()
      e.stopPropagation()
      selectPath(ctx, namePath)
    }
  }, `"${name}"`)
}

function renderGuidNodeHeader(
  ctx: TreeContext,
  node: GuidId,
  path: Path,
  hasChildren: boolean,
  collapsed: boolean,
  selected: boolean
): HTMLDivElement {
  const name = getNodeName(ctx, node)
  const isaName = getNodeIsaName(ctx, node)

  return NodeHeader(selected, () => selectPath(ctx, path),
    (!name && !isaName) ? NodeIdenticon(node) : null,
    renderIsaProjection(ctx, node),
    renderNameProjection(ctx, node, path),
    hasChildren ? CollapseToggle(collapsed, () => ctx.setCollapsed(path, !collapsed)) : null,
    ...renderActionButtons(ctx, node)
  )
}

function renderEdgeLabel(ctx: TreeContext, label: GuidId): HTMLSpanElement {
  const name = getNodeName(ctx, label)
  return name
    ? el('span', { style: { fontSize: '0.85em', color: '#666' } }, name)
    : EdgeLabel(label)
}

function renderChildren(
  ctx: TreeContext,
  path: Path,
  subtree: SpanningTree,
  ancestors: Set<Id>,
  edges: [GuidId, Id][]
): HTMLUListElement {
  const pendingEdge = getPendingEdge(ctx, getSelectedPath(ctx.selection), path, edges)
  return ChildrenList(
    ...edges.map(([edgeLabel, childNode]) => {
      const edgePath = childPath(path, edgeLabel)
      const childSubtree = subtree.children.get(edgeLabel) ?? emptySpanningTree()
      return ChildItem(
        renderEdgeLabel(ctx, edgeLabel),
        TreeNode(ctx, childNode, edgePath, childSubtree, ancestors)
      )
    }),
    ...(pendingEdge ? [ChildItem(
      renderEdgeLabel(ctx, pendingEdge),
      TreeNode(ctx, undefined, childPath(path, pendingEdge), emptySpanningTree(), ancestors)
    )] : [])
  )
}

function isProjectedLabel(ctx: TreeContext, label: GuidId): boolean {
  return (ctx.nameLabel?.equals(label) ?? false) || (ctx.isaLabel?.equals(label) ?? false)
}

function GuidNode(
  ctx: TreeContext,
  node: GuidId,
  path: Path,
  subtree: SpanningTree,
  ancestors: Set<Id>
): HTMLDivElement {
  const { gid } = ctx
  const allEdges = [...gid(node) ?? []]
  const edges = allEdges.filter(([label]) => !isProjectedLabel(ctx, label))
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

function NameLabelSlot(ctx: TreeContext): HTMLDivElement {
  const selected = ctx.selection?.type === 'nameLabel'
  return LabelSlot('name', ctx.nameLabel, selected, () => ctx.select({ type: 'nameLabel' }))
}

function IsaLabelSlot(ctx: TreeContext): HTMLDivElement {
  const selected = ctx.selection?.type === 'isaLabel'
  return LabelSlot('isa', ctx.isaLabel, selected, () => ctx.select({ type: 'isaLabel' }))
}

export function TreeView(ctx: TreeContext): HTMLDivElement {
  const { roots, selection } = ctx

  const labelSlots = el('div', {
    style: { marginBottom: '8px', borderBottom: '1px solid #ddd', paddingBottom: '8px' }
  }, NameLabelSlot(ctx), IsaLabelSlot(ctx))

  if (roots.length === 0) {
    return TreeViewContainer(
      () => ctx.select(undefined),
      labelSlots,
      NewNodeButton(() => ctx.insertRoot(0, ctx.newNode()))
    )
  }

  const elements = roots.flatMap((root, i) => [
    RootInsertionPoint(ctx, i),
    RootSlotView(ctx, root)
  ]).concat([RootInsertionPoint(ctx, roots.length)])

  const newNodeButton = matchSelection(selection, {
    path: p => NewNodeButton(() => setAtPath(ctx, p, ctx.newNode())),
    insertAt: i => NewNodeButton(() => ctx.insertRoot(i, ctx.newNode())),
    nameLabel: () => null,
    isaLabel: () => null,
    none: () => null
  })

  return TreeViewContainer(
    () => ctx.select(undefined),
    labelSlots,
    ...elements,
    newNodeButton
  )
}
