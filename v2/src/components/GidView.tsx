import { For } from 'solid-js'
import type { Id } from '../gid/id'
import { GuidId, StringId, NumberId } from '../gid/id'
import { MutGid } from '../gid/mutgid'
import { Identicon } from './Identicon'

type Props = {
  gid: MutGid
}

function IdView(props: { id: Id, size?: number }) {
  const id = props.id
  if (id instanceof GuidId) {
    return <Identicon value={id.guid} size={props.size} />
  } else if (id instanceof StringId) {
    return <span class="id string">"{id.value}"</span>
  } else if (id instanceof NumberId) {
    return <span class="id number">{id.value}</span>
  }
  return <span class="id unknown">???</span>
}

export function GidView(props: Props) {
  const nodes = () => [...props.gid.nodes()]

  return (
    <div class="gid-view">
      <For each={nodes()}>
        {([node, edges]) => (
          <div class="node">
            <div class="node-header">
              <Identicon value={node.guid} size={24} />
            </div>
            <ul class="edges">
              <For each={edges}>
                {([label, value]) => (
                  <li class="edge">
                    <IdView id={label} /> → <IdView id={value} />
                  </li>
                )}
              </For>
            </ul>
          </div>
        )}
      </For>
    </div>
  )
}
