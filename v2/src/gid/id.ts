import { Maybe, firstMap } from '../maybe'

export interface Id {
  equals(other: Id): boolean
  toJSON(): object
}

function parse<V, T>(s: unknown, key: string, type: string, ctor: new (v: V) => T): Maybe<T> {
  return  typeof s === 'object' && s !== null && key in s && typeof (s as any)[key] === type
    ? new ctor((s as any)[key])
    : undefined
}

export class GuidId implements Id {
  constructor(public readonly guid: string) {}
  equals(other: Id): boolean { return other instanceof GuidId && other.guid === this.guid }
  toJSON() { return { guid: this.guid } }
  static fromJSON(s: unknown): Maybe<GuidId> { return parse(s, 'guid', 'string', GuidId) }
  static generate(): GuidId {
    const s4 = () => Math.floor((1 + Math.random()) * 0x10000).toString(16).substring(1)
    return new GuidId(s4() + s4() + s4() + s4() + s4() + s4() + s4() + s4())
  }
}

export class StringId implements Id {
  constructor(public readonly value: string) {}
  equals(other: Id): boolean { return other instanceof StringId && other.value === this.value }
  toJSON() { return { string: this.value } }
  static fromJSON(s: unknown): Maybe<StringId> { return parse(s, 'string', 'string', StringId) }
}

export class NumberId implements Id {
  constructor(public readonly value: number) {}
  equals(other: Id): boolean { return other instanceof NumberId && other.value === this.value }
  toJSON() { return { number: this.value } }
  static fromJSON(s: unknown): Maybe<NumberId> { return parse(s, 'number', 'number', NumberId) }
}

const idParsers = [GuidId.fromJSON, StringId.fromJSON, NumberId.fromJSON]
export function idFromJSON(s: unknown): Maybe<Id> { return firstMap(idParsers, parser => parser(s)) }
