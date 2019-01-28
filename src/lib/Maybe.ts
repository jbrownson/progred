import {concatMap} from "./Array"
import {assert} from "./assert"
import {compose} from "./compose"

export type Maybe<A> = A | undefined
export const nothing = undefined
export const fromMaybe = <A>(maybeA: Maybe<A>, _default: () => A): A => maybeA === undefined ? _default() : maybeA
export const just = <A>(a: A): Maybe<A> => a
export const maybe = <A, B>(maybeA: Maybe<A>, _default: () => B, f: (a: A) => B): B => maybeA === undefined ? _default() : f(maybeA)
export const maybe2 = <A, B, R>(maybeA: Maybe<A>, maybeB: Maybe<B>, _default: () => R, f: (a: A, b: B) => R): R => (maybeA === undefined || maybeB === undefined) ? _default() : f(maybeA, maybeB)
export const mapMaybe = <A, B>(maybeA: Maybe<A>, f: (a: A) => B): Maybe<B> => maybeA === undefined ? undefined : f(maybeA)
export const map2Maybe = <A,B,C>(maybeA: Maybe<A>, maybeB: Maybe<B>, f: (a:A, b:B) => C): Maybe<C> => {
  if (maybeA !== undefined && maybeB !== undefined)
    return f(maybeA, maybeB)
  return undefined }
export const bindMaybe = <A, B>(maybeA: Maybe<A>, f: (a: A) => Maybe<B>): Maybe<B> => maybeA === undefined ? undefined : f(maybeA)
export const bind2Maybe = <A, B, C>(a: Maybe<A>, b: Maybe<B>, f: (a: A, b: B) => Maybe<C>): Maybe<C> => a === undefined || b === undefined ? undefined : f(a, b)
export const unsafeUnwrapMaybe = <A>(maybe: Maybe<A>): A => { assert(maybe !== nothing); return maybe as A; }
export const sequenceMaybe = <A>(maybes: (() => Maybe<A>)[]): Maybe<A[]> => {
  let as: A[] = []
  for (let f of maybes) {
    let maybeA = f()
    if (maybeA === nothing) return nothing
    as.push(maybeA) }
  return as }
export const maybeToArray = <A>(maybeA: Maybe<A>): A[] => maybeA !== undefined ? [maybeA] : []
export const filterMaybes = <A>(maybes: Maybe<A>[]) => maybeMap(maybes, x => x)
export const filterMaybe = <A>(maybe: Maybe<A>, f: (a: A) => boolean) => bindMaybe(maybe, a => f(a) ? a : nothing)
export const maybeMap = <A, B>(as: A[], f: (a: A) => Maybe<B>): B[] => concatMap(as, compose(f, maybeToArray))
export const maybeReduce = <A, B>(as: A[], f: (b: B, a: A) => Maybe<B>, b: B): Maybe<B> => as.length === 0 ? b : bindMaybe(f(b, as[0]), b => maybeReduce(as.slice(1), f, b))
export const equalMaybe = <A>(a: Maybe<A>, b: Maybe<A>, f: (a: A, b: A) => boolean) => {
  return maybe(a, () => maybe(b, () => true, () => false), a => maybe(b, () => false, b => f(a, b))) }
export const altMaybe = <A>(maybe: Maybe<A>, ...maybes: (() => Maybe<A>)[]): Maybe<A> => { return fromMaybe(maybe, () => firstMaybe(maybes)) }
export const booleanFromMaybe = <A>(maybe: Maybe<A>):boolean => maybe !== undefined
export const firstMaybe = <A>(maybes: (() => Maybe<A>)[]): Maybe<A> => {
  for (let f of maybes) {
    let maybe = f()
    if (maybe !== undefined) return maybe }
  return nothing }
export const maybesEqual = <A>(a: Maybe<A>, b: Maybe<A>, f: (a: A, b: A) => boolean) => maybe(a, () => maybe(b, () => true, b => false), a => maybe(b, () => false, b => f(a, b)))
export const unfold = <A, B>(_a: A, f: (a: A) => {a: Maybe<A>, b: B}): B[] => {
  let {a, b} = f(_a)
  return [b, ...maybe(a, () => [], a => unfold(a, f))] }
export const guardMaybe = (boolean: boolean): Maybe<{}> => boolean ? {} : nothing // TODO use this in places where we're doing return {}
export const maybeFromException = <A>(f: () => A): Maybe<A> => {
  try { return f() } catch(e) {
    console.log(e)
    return nothing }}