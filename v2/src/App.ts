import { el } from './dom'
import { TreeView, emptyTreeViewState } from './components/TreeView'
import type { TreeViewState } from './components/TreeView'
import { MutGid } from './gid/mutgid'
import { GuidId, StringId, NumberId } from './gid/id'
import { cursorNode } from './cursor'
import './App.css'

// Create some test data with a cycle
const testGid = new MutGid()

const alice = GuidId.generate()
const bob = GuidId.generate()
const carol = GuidId.generate()

// Labels
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
testGid.set(carol, friend, alice)  // Cycle back to alice!

export default function App(): HTMLElement {
  let root: GuidId | undefined = alice
  let viewState: TreeViewState = emptyTreeViewState()
  const treeContainer = el('div', {})

  const renderTree = () => {
    const tree = TreeView(testGid.asGid(), root, viewState, {
      onStateChange: (newState) => {
        viewState = newState
        renderTree()
      }
    })
    if (treeContainer.firstChild) {
      treeContainer.replaceChild(tree, treeContainer.firstChild)
    } else {
      treeContainer.appendChild(tree)
    }
  }

  const handleDelete = () => {
    const selection = viewState.selection
    if (selection === undefined) return
    if (selection.type === 'root') {
      root = undefined
    } else {
      const entity = cursorNode(selection.parent, testGid.asGid(), root)
      if (entity instanceof GuidId) {
        testGid.delete(entity, selection.label)
      }
    }
    viewState = { ...viewState, selection: undefined }
    renderTree()
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      viewState = { ...viewState, selection: undefined }
      renderTree()
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      // Don't delete if user is typing in an input
      if (document.activeElement?.tagName === 'INPUT') return
      handleDelete()
    }
  })

  renderTree()

  return el('main', { class: 'container' },
    el('h1', {}, 'gid viewer'),
    treeContainer
  )
}
