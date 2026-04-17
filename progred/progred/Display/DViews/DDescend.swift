import AppKit

class DDescend: FlippedView, Reconcilable {
    var descend: Descend
    var childView: NSView!
    var editor: Editor

    init(_ descend: Descend, editor: Editor, vertical: Bool?) {
        self.descend = descend
        self.editor = editor
        super.init(frame: .zero)
        self.childView = createView(descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical, advance: { [weak self] in self?.advance($0) })
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?) -> Bool {
        guard case .descend(let descend) = d else { return false }
        self.editor = editor
        self.descend = descend
        let resolved = reconcileChild(childView, descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical, advance: { [weak self] in self?.advance($0) })
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self)
            childView = resolved
        }
        return true
    }

    func advance(_ direction: NavigationDirection) {
        nextFocusTarget(direction).flatMap { window?.makeFirstResponder($0) }
    }
}
