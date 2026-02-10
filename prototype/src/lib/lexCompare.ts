export function lexCompare<A>(a0: A, a1: A, f: (a0: A, a1: A) => number, ...fs: ((a0: A, a1: A) => number)[]): number {
  let x = f(a0, a1)
  return x === 0 && fs.length > 0 ? lexCompare(a0, a1, fs[0], ...fs.slice(1)) : x }