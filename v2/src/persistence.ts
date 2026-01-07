import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs'
import { MutGid } from './gid/mutgid'
import { GuidId } from './gid/id'
import type { Maybe } from './maybe'

export type DocumentData = {
  graph: Record<string, Record<string, unknown>>
  root: { guid: string } | null
}

export function serializeDocument(gid: MutGid, root: Maybe<GuidId>): string {
  const data: DocumentData = {
    graph: gid.toJSON(),
    root: root ? { guid: root.guid } : null
  }
  return JSON.stringify(data, null, 2)
}

export function deserializeDocument(json: string): { gid: MutGid, root: Maybe<GuidId> } {
  const data: DocumentData = JSON.parse(json)
  const gid = MutGid.fromJSON(data.graph)
  const root = data.root ? new GuidId(data.root.guid) : undefined
  return { gid, root }
}

export async function saveDocument(gid: MutGid, root: Maybe<GuidId>): Promise<boolean> {
  const path = await save({
    filters: [{ name: 'Progred', extensions: ['progred'] }]
  })
  if (!path) return false
  const content = serializeDocument(gid, root)
  await writeTextFile(path, content)
  return true
}

export async function openDocument(): Promise<{ gid: MutGid, root: Maybe<GuidId> } | null> {
  const path = await open({
    filters: [{ name: 'Progred', extensions: ['progred'] }]
  })
  if (!path) return null
  const content = await readTextFile(path)
  return deserializeDocument(content)
}
