export type Maybe<T> = T | undefined

export function* map<T, R>(items: Iterable<T>, f: (item: T) => R): Iterable<R> {
  for (const item of items) yield f(item)
}

export function firstMap<T, R>(items: Iterable<T>, f: (item: T) => Maybe<R>): Maybe<R> {
  for (const item of items) {
    const result = f(item)
    if (result !== undefined) return result
  }
  return undefined
}

export function traverse<T, R>(items: Iterable<T>, f: (item: T) => Maybe<R>): Maybe<R[]> {
  const results: R[] = []
  for (const item of items) {
    const result = f(item)
    if (result === undefined) return undefined
    results.push(result)
  }
  return results
}
