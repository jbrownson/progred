import { GidView } from './components/GidView'
import { MutGid } from './gid/mutgid'
import { GuidId, StringId, NumberId } from './gid/id'
import './App.css'

const testGid = new MutGid()

const alice = GuidId.generate()
const bob = GuidId.generate()
const name = GuidId.generate()
const age = GuidId.generate()
const friend = GuidId.generate()

testGid.set(alice, name, new StringId('Alice'))
testGid.set(alice, age, new NumberId(30))
testGid.set(alice, friend, bob)

testGid.set(bob, name, new StringId('Bob'))
testGid.set(bob, age, new NumberId(25))
testGid.set(bob, friend, alice)

function App() {
  return (
    <main class="container">
      <h1>gid viewer</h1>
      <GidView gid={testGid} />
    </main>
  )
}

export default App
