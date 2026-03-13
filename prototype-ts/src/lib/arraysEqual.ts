export function arraysEqual<T>(a: T[], b: T[], p: (a: T, b: T) => boolean): boolean {
  if (a === b) return true
  if(a.length !== b.length) return false
  for (let i = 0; i < a.length; ++i) if (!p(a[i], b[i])) return false
  return true }