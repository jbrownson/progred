import AppKit

class DDescend: FlippedView, Reconcilable {
    var descend: Descend
    var childView: NSView!
    var editor: Editor

    init(_ descend: Descend, editor: Editor, vertical: Bool?) {
        self.descend = descend
        self.editor = editor
        super.init(frame: .zero)
        self.childView = createView(descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical, advance: { [weak self] in self?.advance($0) }, focusBody: { [weak self] in self?.focusBody() })
        addSubview(childView)
        constrain(childView, toFill: self)
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?, focusBody: FocusBody?) -> Bool {
        guard case .descend(let descend) = d else { return false }
        self.editor = editor
        self.descend = descend
        let resolved = reconcileChild(childView, descend.body, editor: editor, inCycle: descend.inCycle, commit: descend.commit, expectedType: descend.expectedType, substitution: descend.substitution, vertical: vertical, advance: { [weak self] in self?.advance($0) }, focusBody: { [weak self] in self?.focusBody() })
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

    /// Focus the wrapper of this DDescend's edge value — used after a commit
    /// to "look at what was just made", whether it's the new value's
    /// StructuralNode wrapper (DSelectable) or a placeholder's Pill.
    func focusBody() {
        let target: NSView? = (childView as? StructuralNode)?.isStructural == true
            ? childView
            : childView.firstDescendantStructural()
        target.flatMap { window?.makeFirstResponder($0) }
    }
}
