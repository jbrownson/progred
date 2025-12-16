import { For, Show, createSignal } from 'solid-js'
import type { Id } from '../gid/id'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'
import { MutGid } from '../gid/mutgid'
import type { Cursor } from '../cursor'
import { rootCursor, childCursor, cursorNode, isCycle, matchCursor } from '../cursor'
import type { Maybe } from '../maybe'
import type { SpanningTree } from '../spanningtree'
import { emptySpanningTree, getCollapsed, setCollapsed } from '../spanningtree'

type TreeNodeProps = {
  gid: MutGid
  root: Maybe<GuidId>
  cursor: Cursor
  tree: SpanningTree
  inCycle: boolean
  onToggle: (cursor: Cursor, currentlyCollapsed: boolean) => void
}

function ValueView(props: { id: Id }) {
  const id = props.id
  if (id instanceof StringId) {
    return <span class="value string">"{id.value}"</span>
  } else if (id instanceof NumberId) {
    return <span class="value number">{id.value}</span>
  }
  return null
}

function getEdges(gid: MutGid, node: Id): [GuidId, Id][] {
  if (!(node instanceof GuidId)) return []  // MutGid only stores edges for GuidId
  const edgeMap = gid.edges(node)
  if (!edgeMap) return []
  return [...edgeMap].map(([labelGuid, value]) => [new GuidId(labelGuid), value])
}

function TreeNode(props: TreeNodeProps) {
  const gidFn = props.gid.asGid()
  const node = () => cursorNode(props.cursor, gidFn, props.root)
  const cycle = () => props.inCycle || isCycle(props.cursor, gidFn, props.root)
  const edges = () => {
    const n = node()
    return n ? getEdges(props.gid, n) : []
  }
  const shouldCollapse = () => {
    const explicit = getCollapsed(props.tree, props.cursor)
    return explicit !== undefined ? explicit : cycle()
  }

  const currentNode = node()
  if (!currentNode) {
    // Empty slot
    return <div class="tree-node empty">(empty)</div>
  }

  return (
    <div class="tree-node">
      <div class="tree-node-header" onClick={() => props.onToggle(props.cursor, shouldCollapse())}>
        <Show when={edges().length > 0}>
          <span class="toggle">{shouldCollapse() ? '▶' : '▼'}</span>
        </Show>
        {matchCursor(props.cursor, {
          root: () => null,
          child: (_, label) => <>
            <Identicon value={label.guid} size={18} label />
            <span class="arrow">→</span>
          </>
        })}
        <Show when={currentNode instanceof GuidId} fallback={<ValueView id={currentNode} />}>
          <Identicon value={(currentNode as GuidId).guid} size={20} />
        </Show>
      </div>

      <Show when={!shouldCollapse()}>
        <ul class="tree-node-children">
          <For each={edges()}>
            {([edgeLabel, value]) => (
              <li>
                <Show
                  when={value instanceof GuidId}
                  fallback={
                    <div class="tree-leaf">
                      <Identicon value={edgeLabel.guid} size={18} label />
                      <span class="arrow">→</span>
                      <ValueView id={value} />
                    </div>
                  }
                >
                  <TreeNode
                    gid={props.gid}
                    root={props.root}
                    cursor={childCursor(props.cursor, edgeLabel)}
                    tree={props.tree}
                    inCycle={cycle()}
                    onToggle={props.onToggle}
                  />
                </Show>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  )
}

export function TreeView(props: { gid: MutGid, root: Maybe<GuidId> }) {
  const [tree, setTree] = createSignal(emptySpanningTree())

  const toggle = (cursor: Cursor, currentlyCollapsed: boolean) => {
    setTree(prev => setCollapsed(prev, cursor, !currentlyCollapsed))
  }

  return (
    <div class="tree-view">
      <TreeNode
        gid={props.gid}
        root={props.root}
        cursor={rootCursor}
        tree={tree()}
        inCycle={false}
        onToggle={toggle}
      />
    </div>
  )
}
