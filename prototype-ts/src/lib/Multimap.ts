import {booleanFromMaybe} from "./Maybe"

export class Multimap<K, V> {
  constructor(public map = new Map<K, Set<V>>()) {}
  add(k: K, v: V) {
    let s = this.map.get(k)
    if (!s) { s = new Set; this.map.set(k, s) }
    s.add(v)
    return v }
  delete(k: K, v: V) {
    let set = this.map.get(k)
    if (set) {
      set.delete(v)
      if (set.size === 0) this.map.delete(k) }}
  has(k: K) { return booleanFromMaybe(this.map.get(k)) }
  get(k: K) { return this.map.get(k) || new Set<V>() }
  merge(that: Multimap<K, V>) {
    let newMap = new Multimap(new Map(Array.from(this.map)))
    for (let [k, s] of that.map) for (let v of s) newMap.add(k, v) 
    return newMap }
  count() { return Array.from(this.map).reduce((count, set) => count + set.length, 0) }}
