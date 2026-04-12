import AppKit

class DDescend: FlippedView, Reconcilable {
    var descend: Descend
    var childView: NSView
    var editor: Editor

    init(_ descend: Descend, editor: Editor, vertical: Bool?) {
        self.descend = descend
        self.childView = createView(descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical)
        self.editor = editor
        super.init(frame: .zero)
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?) -> Bool {
        guard case .descend(let descend) = d else { return false }
        self.editor = editor
        self.descend = descend
        let resolved = reconcileChild(childView, descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self)
            childView = resolved
        }
        return true
    }
}
