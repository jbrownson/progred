import { el } from './dom'
import { TreeView, setAtPath } from './components/TreeView'
import type { TreeContext } from './components/TreeView'
import { MutGid } from './gid/mutgid'
import { GuidId, StringId, NumberId } from './gid/id'
import { emptySpanningTree, setCollapsedAtPath } from './spanningtree'
import type { SpanningTree } from './spanningtree'
import type { Maybe } from './maybe'
import type { Path } from './path'
import { saveDocument, openDocument } from './persistence'
import { listen } from '@tauri-apps/api/event'
import './App.css'

function createTestData(): { gid: MutGid, root: GuidId } {
  const gid = new MutGid()
  const alice = GuidId.generate()
  const bob = GuidId.generate()
  const carol = GuidId.generate()
  const name = GuidId.generate()
  const age = GuidId.generate()
  const friend = GuidId.generate()

  gid.set(alice, name, new StringId('Alice'))
  gid.set(alice, age, new NumberId(30))
  gid.set(alice, friend, bob)
  gid.set(bob, name, new StringId('Bob'))
  gid.set(bob, age, new NumberId(25))
  gid.set(bob, friend, carol)
  gid.set(carol, name, new StringId('Carol'))
  gid.set(carol, age, new NumberId(28))
  gid.set(carol, friend, alice)

  return { gid, root: alice }
}

type AppState = {
  gid: MutGid
  root: Maybe<GuidId>
  tree: SpanningTree
  selection: Maybe<Path>
}

function makeContext(state: AppState, rerender: () => void): TreeContext {
  return {
    gid: state.gid.asGid(),
    root: state.root,
    tree: state.tree,
    selection: state.selection,
    setCollapsed: (path, collapsed) => {
      state.tree = setCollapsedAtPath(state.tree, path, collapsed)
      rerender()
    },
    select: path => {
      state.selection = path
      rerender()
    },
    setRoot: value => {
      state.root = value
      state.selection = undefined
      rerender()
    },
    setEdge: (parent, label, value) => {
      state.gid.set(parent, label, value)
      state.selection = undefined
      rerender()
    },
    clearRoot: () => {
      state.root = undefined
      state.selection = undefined
      rerender()
    },
    deleteEdge: (parent, label) => {
      state.gid.delete(parent, label)
      state.selection = undefined
      rerender()
    },
    newNode: () => GuidId.generate()
  }
}

function renderTree(ctx: TreeContext, container: HTMLDivElement): void {
  const tree = TreeView(ctx)
  if (container.firstChild) {
    container.replaceChild(tree, container.firstChild)
  } else {
    container.appendChild(tree)
  }
}

function handleDelete(ctx: TreeContext): void {
  if (ctx.selection) {
    setAtPath(ctx, ctx.selection, undefined)
  }
}

export default function App(): HTMLElement {
  const testData = createTestData()
  const state: AppState = {
    gid: testData.gid,
    root: testData.root,
    tree: emptySpanningTree(),
    selection: undefined
  }
  const treeContainer = el('div', {})
  const rerender = () => {
    const ctx = makeContext(state, rerender)
    renderTree(ctx, treeContainer)
  }

  const handleSave = async () => {
    await saveDocument(state.gid, state.root)
  }

  const handleOpen = async () => {
    const result = await openDocument()
    if (result) {
      state.gid = result.gid
      state.root = result.root
      state.tree = emptySpanningTree()
      state.selection = undefined
      rerender()
    }
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      state.selection = undefined
      rerender()
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      if (document.activeElement?.tagName === 'INPUT') return
      const ctx = makeContext(state, rerender)
      handleDelete(ctx)
    }
  })

  listen('menu-save', () => handleSave())
  listen('menu-open', () => handleOpen())

  rerender()

  return el('main', { style: { margin: 0, padding: '1em' } },
    treeContainer
  )
}
