export type Entry = {
  string: string,
  disambiguation?: string,
  action: () => void,
  matching: boolean,
  external: boolean,
  magic: boolean }