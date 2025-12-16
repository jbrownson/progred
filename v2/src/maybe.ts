export type Maybe<T> = T | undefined

export function firstMap<T, R>(items: Iterable<T>, f: (item: T) => Maybe<R>): Maybe<R> {
  for (const item of items) {
    const result = f(item)
    if (result !== undefined) return result
  }
  return undefined
}
