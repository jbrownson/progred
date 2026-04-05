import AppKit

class DLine: NSStackView, Reconcilable {
    init(children: [D], editor: Editor) {
        super.init(frame: .zero)
        orientation = .horizontal
        alignment = .top
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        children.forEach { addArrangedSubview(createView($0, editor: editor, vertical: false)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, vertical: Bool?) -> Bool {
        guard case .line(let children) = d else { return false }
        reconcileChildren(stack: self, children: children, editor: editor, vertical: false)
        return true
    }
}
