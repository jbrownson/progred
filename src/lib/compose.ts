export function compose<A, B, C>(f0: (a: A) => B, f1: (b: B) => C): (a: A) => C { return a => f1(f0(a)) }
