const scrollListeners = new Set<() => void>()

export function registerScrollListener(listener: () => void) {
  scrollListeners.add(listener)
  return () => scrollListeners.delete(listener)
}

export function notifyScrollListeners() {
  scrollListeners.forEach(listener => listener())
}
