import { el } from './dom'
import { TreeView } from './components/TreeView'
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
  const treeContainer = el('div', {})

  const renderTree = () => {
    const tree = TreeView(testGid.asGid(), root, {
      onDelete: (cursor) => {
        if (cursor.type === 'root') {
          root = undefined
        } else {
          const entity = cursorNode(cursor.parent, testGid.asGid(), root)
          if (entity instanceof GuidId) {
            testGid.delete(entity, cursor.label)
          }
        }
        renderTree()
      }
    })
    if (treeContainer.firstChild) {
      treeContainer.replaceChild(tree, treeContainer.firstChild)
    } else {
      treeContainer.appendChild(tree)
    }
  }

  renderTree()

  return el('main', { class: 'container' },
    el('h1', {}, 'gid viewer'),
    treeContainer
  )
}
