import { patch } from './dom'
import type { VNode } from './dom'
import { TreeView, setAtPath } from './components/TreeView'
import type { TreeContext, RootSlot, Selection } from './components/TreeView'
import { MutGid } from './gid/mutgid'
import { GuidId, StringId } from './gid/id'
import { emptySpanningTree, setCollapsedAtPath } from './spanningtree'
import type { SpanningTree } from './spanningtree'
import type { Maybe } from './maybe'
import { saveDocument, openDocument } from './persistence'
import { listen } from '@tauri-apps/api/event'
import './App.css'

function createTestData(): { gid: MutGid, roots: GuidId[], nameLabel: GuidId, isaLabel: GuidId } {
  const gid = new MutGid()

  // Bootstrap: define 'field' and 'name'/'isa' as fields
  const field = GuidId.generate()
  const name = GuidId.generate()
  const isa = GuidId.generate()

  // field is-a field, named "field"
  gid.set(field, isa, field)
  gid.set(field, name, new StringId('field'))

  // name is-a field, named "name"
  gid.set(name, isa, field)
  gid.set(name, name, new StringId('name'))

  // isa is-a field, named "isa"
  gid.set(isa, isa, field)
  gid.set(isa, name, new StringId('isa'))

  return { gid, roots: [field, name, isa], nameLabel: name, isaLabel: isa }
}

type AppState = {
  gid: MutGid
  roots: RootSlot[]
  tree: SpanningTree
  selection: Maybe<Selection>
  nameLabel: Maybe<GuidId>
  isaLabel: Maybe<GuidId>
}

function makeContext(state: AppState, rerender: () => void): TreeContext {
  return {
    gid: state.gid.asGid(),
    roots: state.roots,
    tree: state.tree,
    selection: state.selection,
    nameLabel: state.nameLabel,
    isaLabel: state.isaLabel,
    setCollapsed: (path, collapsed) => {
      state.tree = setCollapsedAtPath(state.tree, path, collapsed)
      rerender()
    },
    select: selection => {
      state.selection = selection
      rerender()
    },
    insertRoot: (index, node) => {
      const id = GuidId.generate()
      state.roots.splice(index, 0, { id, node })
      state.selection = undefined
      rerender()
    },
    setRootNode: (slotId, node) => {
      const rootSlot = state.roots.find(r => r.id.equals(slotId))
      if (rootSlot) {
        rootSlot.node = node
        state.selection = undefined
        rerender()
      }
    },
    deleteRoot: slotId => {
      state.roots = state.roots.filter(r => !r.id.equals(slotId))
      state.selection = undefined
      rerender()
    },
    setEdge: (parent, label, value) => {
      state.gid.set(parent, label, value)
      rerender()
    },
    deleteEdge: (parent, label) => {
      state.gid.delete(parent, label)
      state.selection = undefined
      rerender()
    },
    setNameLabel: label => {
      state.nameLabel = label
      state.selection = undefined
      rerender()
    },
    setIsaLabel: label => {
      state.isaLabel = label
      state.selection = undefined
      rerender()
    },
    newNode: () => GuidId.generate()
  }
}

function renderTree(ctx: TreeContext, currentVNode: VNode | Element): VNode {
  const newVNode = TreeView(ctx)
  patch(currentVNode, newVNode)
  return newVNode
}

function handleDelete(ctx: TreeContext): void {
  switch (ctx.selection?.type) {
    case 'path':
      setAtPath(ctx, ctx.selection.path, undefined)
      break
    case 'nameLabel':
      ctx.setNameLabel(undefined)
      break
    case 'isaLabel':
      ctx.setIsaLabel(undefined)
      break
  }
}

export default function App(): HTMLElement {
  const testData = createTestData()
  const state: AppState = {
    gid: testData.gid,
    roots: testData.roots.map(node => ({ id: GuidId.generate(), node })),
    tree: emptySpanningTree(),
    selection: undefined,
    nameLabel: testData.nameLabel,
    isaLabel: testData.isaLabel
  }
  const container = document.createElement('div')
  let currentVNode: VNode | Element = container
  let renderScheduled = false
  const scheduleRender = () => {
    if (renderScheduled) return
    renderScheduled = true
    setTimeout(() => {
      renderScheduled = false
      const ctx = makeContext(state, scheduleRender)
      currentVNode = renderTree(ctx, currentVNode)
    }, 0)
  }

  const handleSave = async () => {
    await saveDocument(state.gid, state.roots)
  }

  const handleOpen = async () => {
    const result = await openDocument()
    if (result) {
      state.gid = result.gid
      state.roots = result.roots
      state.tree = emptySpanningTree()
      state.selection = undefined
      scheduleRender()
    }
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      state.selection = undefined
      scheduleRender()
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      if (document.activeElement?.tagName === 'INPUT') return
      const ctx = makeContext(state, scheduleRender)
      handleDelete(ctx)
    }
  })

  listen('menu-save', () => handleSave())
  listen('menu-open', () => handleOpen())

  scheduleRender()

  return container
}
