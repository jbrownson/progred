import type { ID } from './id'
import { Maybe } from '../maybe'

export type Graph = (node: ID, label: ID) => Maybe<ID>
export const empty: Graph = () => undefined
export function compose(a: Graph, b: Graph): Graph { return (node, label) => a(node, label) ?? b(node, label) }
export function composeAll(...graphs: Graph[]): Graph { return graphs.reduce(compose, empty) }
