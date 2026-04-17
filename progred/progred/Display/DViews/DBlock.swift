import AppKit

class DBlock: NSStackView, Reconcilable {
    init(children: [D], editor: Editor, advance: Advance?) {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        children.forEach { addArrangedSubview(createView($0, editor: editor, vertical: true, advance: advance)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, expectedType: Id?, substitution: Substitution, vertical: Bool?, advance: Advance?) -> Bool {
        guard case .block(let children) = d else { return false }
        reconcileChildren(stack: self, children: children, editor: editor, vertical: true, advance: advance)
        return true
    }
}
