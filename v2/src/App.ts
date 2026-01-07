import { el } from './dom'
import { TreeView, setAtPath } from './components/TreeView'
import type { TreeContext } from './components/TreeView'
import { MutGid } from './gid/mutgid'
import { GuidId, StringId, NumberId } from './gid/id'
import { emptySpanningTree, setCollapsedAtPath } from './spanningtree'
import type { SpanningTree } from './spanningtree'
import type { Maybe } from './maybe'
import type { Path } from './path'
import './App.css'

const testGid = new MutGid()

const alice = GuidId.generate()
const bob = GuidId.generate()
const carol = GuidId.generate()

const name = GuidId.generate()
const age = GuidId.generate()
const friend = GuidId.generate()

testGid.set(alice, name, new StringId('Alice'))
testGid.set(alice, age, new NumberId(30))
testGid.set(alice, friend, bob)

testGid.set(bob, name, new StringId('Bob'))
testGid.set(bob, age, new NumberId(25))
testGid.set(bob, friend, carol)

testGid.set(carol, name, new StringId('Carol'))
testGid.set(carol, age, new NumberId(28))
testGid.set(carol, friend, alice)  // Cycle

type AppState = {
  root: Maybe<GuidId>
  tree: SpanningTree
  selection: Maybe<Path>
}

function makeContext(
  gid: MutGid,
  state: AppState,
  rerender: () => void
): TreeContext {
  return {
    gid: gid.asGid(),
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
      gid.set(parent, label, value)
      state.selection = undefined
      rerender()
    },
    clearRoot: () => {
      state.root = undefined
      state.selection = undefined
      rerender()
    },
    deleteEdge: (parent, label) => {
      gid.delete(parent, label)
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
  const state: AppState = {
    root: alice,
    tree: emptySpanningTree(),
    selection: undefined
  }
  const treeContainer = el('div', {})
  const rerender = () => {
    const ctx = makeContext(testGid, state, rerender)
    renderTree(ctx, treeContainer)
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      state.selection = undefined
      rerender()
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      if (document.activeElement?.tagName === 'INPUT') return
      const ctx = makeContext(testGid, state, rerender)
      handleDelete(ctx)
    }
  })

  rerender()

  return el('main', { style: { margin: 0, padding: '1em' } },
    el('h1', { style: { marginTop: 0 } }, 'gid viewer'),
    treeContainer
  )
}
