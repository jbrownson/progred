import AppKit

class EditorWindow: NSWindow {
    override var acceptsFirstResponder: Bool { true }

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
        case .backtab:
            if let last = contentView?.lastFocusTarget() {
                makeFirstResponder(last)
            }
        case .tab:
            break
        }
    }
}

@main
class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!
    private var editor: Editor!
    private var rootView: RootView!

    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.mainMenu = buildMainMenu(target: delegate)
        app.setActivationPolicy(.regular)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let schema = Schema.bootstrap()
        let (document, root) = Editor.sampleDocument(schema)
        editor = Editor(
            schema: schema, document: document, root: root,
            onChange: { [weak self] delta in self?.rootView.apply(delta) })
        rootView = RootView(editor: editor)

        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = false
        scrollView.documentView = rootView
        NSLayoutConstraint.activate([
            rootView.leadingAnchor.constraint(equalTo: scrollView.contentView.leadingAnchor),
            rootView.topAnchor.constraint(equalTo: scrollView.contentView.topAnchor),
            rootView.widthAnchor.constraint(greaterThanOrEqualTo: scrollView.contentView.widthAnchor),
        ])

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
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    @objc func newDocument(_ sender: Any?) {
        editor.replace(document: MutGid(), root: nil)
    }

    @objc func loadSampleDocument(_ sender: Any?) {
        let (document, root) = Editor.sampleDocument(editor.schema)
        editor.replace(document: document, root: root)
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
