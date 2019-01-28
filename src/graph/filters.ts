import { stableSort } from "../lib/Array"
import { mapMaybe, Maybe, maybe, maybeReduce, nothing } from "../lib/Maybe"

export type Match = {start: number, length: number}

export type Filter<A> = (as: A[], f: (a: A) => string, needle: string) => {accepted: {a: A, matches: Match[]}[], rejected: A[]}

export function acceptAll<A>(as: A[], f: (a: A) => string) {
  return {accepted: as.map(a => ({a, matches: [{start: 0, length: f(a).length}]})), rejected: []} }

export function rejectAll<A>(as: A[], f: (a: A) => string) { return {accepted: [], rejected: as} }

export function orFilters<A>(...filters: Filter<A>[]): Filter<A> {
  switch (filters.length) {
    case 0: return rejectAll
    case 1: return filters[0]
    default: return orFilter(filters[0], orFilters(...filters.slice(1))) }}

export function orFilter<A>(filterA: Filter<A>, filterB: Filter<A>): Filter<A> {
  return (as, f, needle) => {
    let a = filterA(as, f, needle)
    let b = filterB(a.rejected, f, needle)
    return {accepted: [...a.accepted, ...b.accepted], rejected: b.rejected} } }

export function caseInsensitiveFilter<A>(filter: Filter<A>): Filter<A> {
  return (as, f, needle) => filter(as, a => f(a).toLocaleLowerCase(), needle.toLocaleLowerCase()) }

export function predicateFilter<A>(predicate: (needle: string, haystack: string) => Maybe<Match[]>): Filter<A> {
  return (as, f, needle) => as.reduce((acceptedRejected, a) => {
    return maybe(predicate(needle, f(a)),
      () => ({accepted: acceptedRejected.accepted, rejected: acceptedRejected.rejected.concat(a)}),
      matches => ({accepted: acceptedRejected.accepted.concat({a, matches}), rejected: acceptedRejected.rejected}) )},
    {accepted: [] as {a: A, matches: Match[]}[], rejected: [] as A[]} )}

type CompareFunction<A> = (a0: {a: A, haystack: string, matches: Match[]}, a1: {a: A, haystack: string, matches: Match[]}) => number
export function sortFilter<A>(filter: Filter<A>, compareFunction: CompareFunction<A>): Filter<A> {
  return (as, f, needle) => {
    let x = filter(as, f, needle)
    return {
      accepted: stableSort(x.accepted, (a, b) => compareFunction({a: a.a, haystack: f(a.a), matches: a.matches}, {a: b.a, haystack: f(b.a), matches: b.matches})),
      rejected: x.rejected }}}

export function compareByPercentMatched(a: {haystack: string, matches: Match[]}, b: {haystack: string, matches: Match[]}): number {
  function totalMatchedChars(matches: Match[]) { return matches.reduce((total, match) => total + match.length, 0) }
  function percentMatched(haystack: string, matches: Match[]) { return totalMatchedChars(matches) / haystack.length }
  return percentMatched(b.haystack, b.matches) - percentMatched(a.haystack, a.matches) }

export function prefixFilter<A>(): Filter<A> {
  return sortFilter<A>(predicateFilter<A>((needle, haystack) => haystack.startsWith(needle) ? [{start: 0, length: needle.length}] : nothing), compareByPercentMatched) }

export function substringFilter<A>(): Filter<A> {
  return sortFilter(predicateFilter<A>((needle, haystack) => { let index = haystack.indexOf(needle); return index >= 0 ? [{start: index, length: needle.length}] : nothing }), compareByPercentMatched) }

export function fuzzyFilter<A>(): Filter<A> {
  return sortFilter(
    predicateFilter<A>((needle, haystack) => mapMaybe(
      maybeReduce(needle.split(""),
        (a: {index: number, matches: number[]}, char) => {
          let index = haystack.indexOf(char, a.index)
          return index >= 0 ? {index: index + 1, matches: a.matches.concat(index)} : nothing },
        {index: 0, matches: []} ),
      x => x.matches.map(match => ({start: match, length: 1})) )),
    compareByPercentMatched )}

export function acceptAllWithEmptyNeedle<A>(): Filter<A> { return (as, f, needle) => needle.length > 0 ? rejectAll(as, f) : acceptAll(as, f) }

export function defaultFilter<A>(): Filter<A> {
  return orFilters(acceptAllWithEmptyNeedle<A>(), prefixFilter<A>(), substringFilter<A>(), caseInsensitiveFilter<A>(prefixFilter<A>()), caseInsensitiveFilter<A>(substringFilter<A>()),
    fuzzyFilter<A>(), caseInsensitiveFilter<A>(fuzzyFilter<A>()) )}