export function lexicographicalCompare<A>(a0: A, a1: A, compares: ((a0: A, a1: A) => number)[]): number {
  for (let compare of compares) {
    let x = compare(a0, a1)
    if (x !== 0) return x }
  return 0 }