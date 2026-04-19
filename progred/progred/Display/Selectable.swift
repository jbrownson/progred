import AppKit

typealias Commit = (Id?) -> Void

final class Selectable: FlippedView {
    let commit: Commit?

    init(_ child: NSView, commit: Commit? = nil) {
        self.commit = commit
        super.init(frame: .zero)
        addSubview(child)
        constrain(child, toFill: self)
    }
    required init?(coder: NSCoder) { fatalError() }

    override var acceptsFirstResponder: Bool { true }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
    }
    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok { setFocusIndicator(true) }
        return ok
    }
    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { setFocusIndicator(false) }
        return ok
    }

    override func keyDown(with event: NSEvent) { interpretKeyEvents([event]) }
    override func deleteBackward(_ sender: Any?) { commit?(nil) }
    override func deleteForward(_ sender: Any?) { commit?(nil) }
}
