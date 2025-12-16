import type { Id } from './id'
import { Maybe } from '../maybe'

export type Gid = (node: Id, label: Id) => Maybe<Id>
export const empty: Gid = () => undefined
export function compose(a: Gid, b: Gid): Gid { return (node, label) => a(node, label) ?? b(node, label) }
export function composeAll(...gids: Gid[]): Gid { return gids.reduce(compose, empty) }
