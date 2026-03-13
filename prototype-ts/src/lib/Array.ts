import { lexCompare } from "./lexCompare"
import { booleanFromMaybe } from "./Maybe"
import {Multimap} from "./Multimap"

export function concatMap<A,B>(as : A[], f: (a: A, i: number) => B[]) : B[] { return new Array<B>().concat(...as.map(f)) }

export function zip<A, B>(as: A[], bs: B[]): [A, B][] {
  return as.length > bs.length
    ? bs.map<[A, B]>((b, i) => [as[i], b])
    : as.map<[A, B]>((a, i) => [a, bs[i]]) }

export function setDifference<A>(xs: A[], ys: A[], equal = (lhs: A, rhs: A) => lhs === rhs): A[] {
  return xs.filter(x => !booleanFromMaybe(ys.find(y => equal(x, y)))) }

export function removeDupes<A>(as: A[]): A[] { return Array.from(new Set(as)) }
export function removeDupesBy<A, B>(as: A[], f: (a: A) => B): A[] {
  let newAs = new Array<A>()
  let bs = new Set<B>()
  for (let a of as) {
    let b = f(a)
    if (!bs.has(b)) {
      bs.add(b)
      newAs.push(a) }}
  return newAs }
export function intersperse<A>(as: A[], a: (index: number) => A): A[] { return concatMap(as, (e, i) => i === 0 ? [e] : [a(i), e]) }
export function join<A>(as: A[][]): A[] { return new Array<A>().concat(...as) }
export function bindArray<A, B>(as: A[], f: (a: A) => B[]): B[] { return join(as.map(f)) }

export function groupBy<A>(as: A[], p: (a: A) => boolean): {trues: A[], falses: A[]} {
  let trues: A[] = []
  let falses: A[] = []
  for (let a of as) (p(a) ? trues : falses).push(a)
  return {trues, falses} }

export function groupByK<K, V>(vs: V[], f: (a: V) => K): Multimap<K, V> {
  return vs.map(v => ({k: f(v), v})).reduce((m, {k, v}) => {m.add(k, v); return m}, new Multimap<K, V>()) }

export function stableSort<A>(as: A[], compare: (a0: A, a1: A) => number) {
  return as.map((a, i) => ({a, i})).sort((l, r) => lexCompare(l, r, ({a: a0}, {a: a1}) => compare(a0, a1), ({i: i0}, {i: i1}) => i0 - i1)).map(({a, i}) => a) }