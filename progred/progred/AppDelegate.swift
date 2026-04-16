import AppKit

class EditorWindow: NSWindow {
    override func keyDown(with event: NSEvent) {
        interpretKeyEvents([event])
    }

    override func insertTab(_ sender: Any?) { advance(.tab) }
    override func insertBacktab(_ sender: Any?) { advance(.backtab) }
    override func moveUp(_ sender: Any?) { advance(.up) }
    override func moveDown(_ sender: Any?) { advance(.down) }
    override func moveLeft(_ sender: Any?) { advance(.left) }
    override func moveRight(_ sender: Any?) { advance(.right) }

    private func advance(_ direction: NavigationDirection) {
        let start: NSView = (firstResponder as? NSView) ?? contentView!
        if let target = start.nextFocusTarget(direction) {
            makeFirstResponder(target)
            return
        }
        switch direction {
        case .up, .down, .left, .right:
            if let root = contentView?.firstDescendantStructural() {
                makeFirstResponder(root)
            }
        case .tab, .backtab:
            break
        }
    }
}

@main
class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!
    private var editor: Editor!
    private var rootView: DRootView!

    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.mainMenu = buildMainMenu(target: delegate)
        app.setActivationPolicy(.regular)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        editor = Editor(schema: Editor.withSampleDocument().schema)

        wireEditor()
        rootView = DRootView(editor: editor)

        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.verticalScrollElasticity = .none
        scrollView.horizontalScrollElasticity = .none
        scrollView.documentView = rootView

        window = EditorWindow(
            contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false)
        window.title = "progred"
        window.contentView = scrollView
        window.autorecalculatesKeyViewLoop = false
        window.center()
        window.makeKeyAndOrderFront(nil)

        rebuild()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    @objc func newDocument(_ sender: Any?) {
        editor = Editor(schema: editor.schema)
        wireEditor()
        rootView.editor = editor
        rebuild()
    }

    @objc func loadSampleDocument(_ sender: Any?) {
        editor = Editor.withSampleDocument()
        wireEditor()
        rootView.editor = editor
        rebuild()
    }

    private func wireEditor() {
        editor.onMutate = { [weak self] in self?.rebuild() }
    }

    private func rebuild() {
        let rootCommit: Commit = { editor, id in editor.root = id }
        let body: D = editor.root.map { root in
            let ctx = ProjectionContext(
                entity: root, gid: editor.gid,
                schema: editor.schema, editor: editor, ancestors: [],
                commit: rootCommit)
            return project(ctx)
        } ?? .placeholder
        let d: D = .descend(Descend(
            inCycle: false,
            commit: rootCommit,
            expectedType: nil,
            substitution: [:],
            body: body))
        rootView.rebuild(d)
    }
}

func buildMainMenu(target: AnyObject) -> NSMenu {
    let mainMenu = NSMenu()

    let appMenuItem = NSMenuItem()
    mainMenu.addItem(appMenuItem)
    let appMenu = NSMenu()
    appMenuItem.submenu = appMenu
    appMenu.addItem(withTitle: "About progred", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
    appMenu.addItem(.separator())
    appMenu.addItem(withTitle: "Quit progred", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

    let fileMenuItem = NSMenuItem()
    mainMenu.addItem(fileMenuItem)
    let fileMenu = NSMenu(title: "File")
    fileMenuItem.submenu = fileMenu
    let newItem = fileMenu.addItem(withTitle: "New", action: #selector(AppDelegate.newDocument(_:)), keyEquivalent: "n")
    newItem.target = target
    let sampleItem = fileMenu.addItem(withTitle: "Load Sample Document", action: #selector(AppDelegate.loadSampleDocument(_:)), keyEquivalent: "")
    sampleItem.target = target

    let editMenuItem = NSMenuItem()
    mainMenu.addItem(editMenuItem)
    let editMenu = NSMenu(title: "Edit")
    editMenuItem.submenu = editMenu
    editMenu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
    editMenu.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
    editMenu.addItem(.separator())
    editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
    editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
    editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
    editMenu.addItem(withTitle: "Delete", action: #selector(NSText.delete(_:)), keyEquivalent: "")
    editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")

    return mainMenu
}
