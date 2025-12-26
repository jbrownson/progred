import { Map } from 'immutable'
import type { Id, GuidId } from './id'
import { Maybe } from '../maybe'

export type Gid = (entity: Id) => Maybe<Map<GuidId, Id>> // TODO should this be Maybe<Map<Id, Id>>?
export const empty: Gid = () => undefined
export function compose(a: Gid, b: Gid): Gid {
  return entity => {
    const aEdges = a(entity)
    const bEdges = b(entity)
    if (!aEdges) return bEdges
    if (!bEdges) return aEdges
    return aEdges.merge(bEdges)
  }
}
export function composeAll(...gids: Gid[]): Gid { return gids.reduce(compose, empty) }
