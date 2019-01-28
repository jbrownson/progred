import {bindMaybe, Maybe, maybe} from "./Maybe"

export class Map2<K0, K1, V> {
  constructor(private map = new Map<K0, Map<K1, V>>()) {}
  get(k0: K0, k1: K1): Maybe<V> { return bindMaybe(this.map.get(k0), map => map.get(k1)) }
  set(k0: K0, k1: K1, v: V) { maybe(this.map.get(k0), () => { this.map.set(k0, new Map([[k1, v]])) }, m => { m.set(k1, v) }) }
  merge(that: Map2<K0, K1, V>) { for (let [k0, map] of that.map) for (let [k1, v] of map) this.set(k0, k1, v) }}