import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs'
import { MutGid } from './gid/mutgid'
import { GuidId } from './gid/id'
import type { RootSlot } from './components/TreeView'

type RootSlotData = { id: string, node: string }

export type DocumentData = {
  graph: Record<string, Record<string, unknown>>
  roots: RootSlotData[]
}

export function serializeDocument(gid: MutGid, roots: RootSlot[]): string {
  const data: DocumentData = {
    graph: gid.toJSON(),
    roots: roots.map(r => ({ id: r.id.guid, node: r.node.guid }))
  }
  return JSON.stringify(data, null, 2)
}

export function deserializeDocument(json: string): { gid: MutGid, roots: RootSlot[] } {
  const data: DocumentData = JSON.parse(json)
  const gid = MutGid.fromJSON(data.graph)
  const roots: RootSlot[] = data.roots.map(r => ({
    id: new GuidId(r.id),
    node: new GuidId(r.node)
  }))
  return { gid, roots }
}

export async function saveDocument(gid: MutGid, roots: RootSlot[]): Promise<boolean> {
  const path = await save({
    filters: [{ name: 'Progred', extensions: ['progred'] }]
  })
  if (!path) return false
  const content = serializeDocument(gid, roots)
  await writeTextFile(path, content)
  return true
}

export async function openDocument(): Promise<{ gid: MutGid, roots: RootSlot[] } | null> {
  const path = await open({
    filters: [{ name: 'Progred', extensions: ['progred'] }]
  })
  if (!path) return null
  const content = await readTextFile(path)
  return deserializeDocument(content)
}
