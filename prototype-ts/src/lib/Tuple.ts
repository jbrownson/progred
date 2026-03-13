import { map2Maybe, mapMaybe, Maybe } from "./Maybe"

export type Tuple<A, B> = [A, B]

export function tuple<A,B>(a: A, b: B): [A,B] { return [a,b] }
export function mapFirst<A,B,C>(tup: [A,B], f: (a: A) => C): [C,B] { return applyTuple([f, x => x], tup) }
export function mapSecond<A,B,C>(tup: [A,B], f: (b: B) => C): [A,C] { return applyTuple([x => x, f], tup) }

export function extractFromTuple<A,B,C>(tup: [A,B], f:(a:A,b:B) => C): C { return f(tup[0], tup[1])}

export function zipTuplesWith<A,B,C,D,E,F>(tup1: [A,B], tup2: [C,D], f: (a:A,c:C) => E, g: (b:B,d:D) => F): [E,F] {
  return [f(tup1[0], tup2[0]), g(tup1[1],tup2[1])] }

export function zipTuples<A,B,C,D>(tup1: [A,B], tup2: [C,D]): [[A,C], [B,D]] {
  return zipTuplesWith(tup1, tup2, tuple, tuple)}

export function applyTuple<A,B,C,D>(funcs: [(a:A) => C, (b:B) => D], tup: [A,B]): [C,D] {
  return zipTuplesWith(funcs, tup, (f,a) => f(a), (f,b) => f(b))}

export function pullFirstMaybe<A,B>(tup: [Maybe<A>, B]): Maybe<[A,B]> {
  return extractFromTuple(tup, (ma,b) => mapMaybe(ma, a => tuple(a,b))) }

export function pullSecondMaybe<A,B>(tup: [A, Maybe<B>]): Maybe<[A,B]> {
  return extractFromTuple(tup, (a,mb) => mapMaybe(mb, b => tuple(a,b)))}

export function extractTupleMaybes<A,B,C>(tup: [Maybe<A>, Maybe<B>], f:(a:A, b:B) => C): Maybe<C> {
  return extractFromTuple(tup, (ma,mb) => map2Maybe(ma, mb, f)) }

export function sequenceMaybeTuple<A,B>(tup: [Maybe<A>, Maybe<B>]): Maybe<[A,B]> {
  return extractTupleMaybes(tup, tuple) }