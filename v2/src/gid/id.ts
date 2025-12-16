import { Maybe, firstMap } from '../maybe'

export interface ID {
  equals(other: ID): boolean
  toJSON(): object
}

function parse<V, T>(s: unknown, key: string, type: string, ctor: new (v: V) => T): Maybe<T> {
  return  typeof s === 'object' && s !== null && key in s && typeof (s as any)[key] === type
    ? new ctor((s as any)[key])
    : undefined
}

export class GuidID implements ID {
  constructor(public readonly guid: string) {}
  equals(other: ID): boolean { return other instanceof GuidID && other.guid === this.guid }
  toJSON() { return { guid: this.guid } }
  static fromJSON(s: unknown): Maybe<GuidID> { return parse(s, 'guid', 'string', GuidID) }
}

export class StringID implements ID {
  constructor(public readonly value: string) {}
  equals(other: ID): boolean { return other instanceof StringID && other.value === this.value }
  toJSON() { return { string: this.value } }
  static fromJSON(s: unknown): Maybe<StringID> { return parse(s, 'string', 'string', StringID) }
}

export class NumberID implements ID {
  constructor(public readonly value: number) {}
  equals(other: ID): boolean { return other instanceof NumberID && other.value === this.value }
  toJSON() { return { number: this.value } }
  static fromJSON(s: unknown): Maybe<NumberID> { return parse(s, 'number', 'number', NumberID) }
}

const idParsers = [GuidID.fromJSON, StringID.fromJSON, NumberID.fromJSON]
export function idFromJSON(s: unknown): Maybe<ID> { return firstMap(idParsers, parser => parser(s)) }

export function generateGuid(): GuidID {
  const s4 = () => Math.floor((1 + Math.random()) * 0x10000).toString(16).substring(1)
  return new GuidID(s4() + s4() + s4() + s4() + s4() + s4() + s4() + s4())
}