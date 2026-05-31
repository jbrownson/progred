import { contextBridge, ipcRenderer } from "electron"

type FileFilter = { name: string, extensions: string[] }
type OpenFileResult = { path: string, contents: string }

function runInContext(javascript: string, sandboxObject: Record<string, unknown>): unknown {
  return Function("sandbox", "javascript", "with (sandbox) { return eval(javascript) }")(sandboxObject, javascript)
}

const progred = {
  openFile: (): Promise<OpenFileResult | undefined> =>
    ipcRenderer.invoke("dialog:open-file"),

  saveFileAs: (contents: string, filters?: FileFilter[]): Promise<string | undefined> =>
    ipcRenderer.invoke("dialog:save-file", { contents, filters }),

  writeFile: (path: string, contents: string): Promise<void> =>
    ipcRenderer.invoke("file:write", { path, contents }),

  readFileBytes: (path: string): Promise<Uint8Array> =>
    ipcRenderer.invoke("file:read-bytes", path),

  writeClipboardText: (format: string, text: string) => {
    ipcRenderer.sendSync("clipboard:write-text", { format, text })
  },

  readClipboardText: (format: string): string | undefined => {
    return ipcRenderer.sendSync("clipboard:read-text", format)
  },

  availableClipboardFormats: (): string[] => ipcRenderer.sendSync("clipboard:available-formats"),

  readPlainText: (): string => ipcRenderer.sendSync("clipboard:read-plain-text"),

  runJavascript: (javascript: string, sandboxObject: Record<string, unknown> = {}): unknown =>
    runInContext(javascript, sandboxObject),

  sendActionToFirstResponder: (action: string) => {
    ipcRenderer.send("menu:send-action-to-first-responder", action)
  },

  setMenuItemEnabled: (id: string, enabled: boolean) => {
    ipcRenderer.send("menu:set-enabled", { id, enabled })
  },

  setMenuItemChecked: (id: string, checked: boolean) => {
    ipcRenderer.send("menu:set-checked", { id, checked })
  },

  onMenuAction: (callback: (action: string) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, action: string) => callback(action)
    ipcRenderer.on("menu:action", listener)
    return () => ipcRenderer.off("menu:action", listener)
  },
}

contextBridge.exposeInMainWorld("progred", progred)

export type ProgredApi = typeof progred
