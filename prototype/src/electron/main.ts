import * as E from 'electron'

E.app.on('ready', () => {
  let browserWindow = new E.BrowserWindow()

  browserWindow.loadURL(`file://${__dirname}/../grapheditor.html`) })