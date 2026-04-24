import { app, BrowserWindow, dialog, ipcMain, Menu } from "electron"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"

let browserWindow: BrowserWindow | undefined

const progredFilters = [{ name: "progred", extensions: ["progred"] }]

function sendMenuAction(action: string) {
  browserWindow?.webContents.send("menu:action", action)
}

function buildMainMenu() {
  return Menu.buildFromTemplate([
    { label: app.name, submenu: [{ role: "about" }, { role: "quit" }] },
    {
      label: "File",
      submenu: [
        { label: "New", accelerator: "CmdOrCtrl+N", click: () => sendMenuAction("new") },
        { label: "New View", click: () => sendMenuAction("new-view") },
        { label: "View Constructor", click: () => sendMenuAction("view-constructor") },
        { type: "separator" },
        { label: "Open...", accelerator: "CmdOrCtrl+O", click: () => sendMenuAction("open") },
        { type: "separator" },
        { label: "Save", accelerator: "CmdOrCtrl+S", click: () => sendMenuAction("save") },
        { label: "Save As...", accelerator: "CmdOrCtrl+Shift+S", click: () => sendMenuAction("save-as") },
        { type: "separator" },
        { label: "Export Text...", accelerator: "CmdOrCtrl+Shift+T", click: () => sendMenuAction("export-text") },
      ],
    },
    {
      label: "Edit",
      submenu: [
        { label: "Undo", accelerator: "CmdOrCtrl+Z", click: () => sendMenuAction("undo") },
        { label: "Redo", accelerator: "Shift+CmdOrCtrl+Z", click: () => sendMenuAction("redo") },
        { type: "separator" },
        { label: "Cut", accelerator: "CmdOrCtrl+X", click: () => sendMenuAction("cut") },
        { label: "Copy", accelerator: "CmdOrCtrl+C", click: () => sendMenuAction("copy") },
        { label: "Paste Structure", accelerator: "CmdOrCtrl+Shift+V", click: () => sendMenuAction("paste-structure") },
        { label: "Paste Reference", accelerator: "CmdOrCtrl+V", click: () => sendMenuAction("paste-reference") },
        { label: "Select All", accelerator: "CmdOrCtrl+A", click: () => sendMenuAction("select-all") },
      ],
    },
    {
      label: "Debug",
      submenu: [
        { label: "Refresh", accelerator: "CmdOrCtrl+R", click: () => browserWindow?.reload() },
        { label: "Open Dev Tools", accelerator: "CmdOrCtrl+Shift+I", click: () => browserWindow?.webContents.openDevTools() },
        { label: "Console Log Selection", accelerator: "CmdOrCtrl+D", click: () => sendMenuAction("console-log-selection") },
      ],
    },
    {
      label: "View",
      submenu: [
        { label: "Collapse", accelerator: "CmdOrCtrl+Shift+[", click: () => sendMenuAction("collapse") },
      ],
    },
    {
      label: "Transforms",
      submenu: [
        { label: "Brad Params -> string", click: () => sendMenuAction("transform-brad-params-string") },
        { label: "string -> JSON", click: () => sendMenuAction("transform-string-json") },
        { label: "JSON -> Brad Params", click: () => sendMenuAction("transform-json-brad-params") },
        { label: "Brad Params -> JSON", click: () => sendMenuAction("transform-brad-params-json") },
        { label: "JSON -> string", click: () => sendMenuAction("transform-json-string") },
      ],
    },
  ])
}

function createWindow() {
  Menu.setApplicationMenu(buildMainMenu())

  browserWindow = new BrowserWindow({
    width: 1000,
    height: 700,
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  const devServerUrl = process.env.VITE_DEV_SERVER_URL
  if (devServerUrl) {
    browserWindow.loadURL(devServerUrl)
  } else {
    browserWindow.loadFile(path.join(__dirname, "renderer", "grapheditor.html"))
  }
}

app.whenReady().then(createWindow)

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit()
})

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow()
})

ipcMain.handle("dialog:open-file", async () => {
  const result = await dialog.showOpenDialog({ filters: progredFilters, properties: ["openFile"] })
  if (result.canceled || result.filePaths.length === 0) return undefined
  const filePath = result.filePaths[0]
  return { path: filePath, contents: await readFile(filePath, "utf8") }
})

ipcMain.handle("dialog:save-file", async (_event, { contents, filters }: { contents: string, filters?: Electron.FileFilter[] }) => {
  const result = await dialog.showSaveDialog({ filters: filters ?? progredFilters })
  if (result.canceled || !result.filePath) return undefined
  await writeFile(result.filePath, contents)
  return result.filePath
})

ipcMain.handle("file:write", async (_event, { path: filePath, contents }: { path: string, contents: string }) => {
  await writeFile(filePath, contents)
})

ipcMain.on("menu:send-action-to-first-responder", (_event, action: string) => {
  const sendAction = (Menu as any).sendActionToFirstResponder
  if (typeof sendAction === "function") sendAction(action)
})
