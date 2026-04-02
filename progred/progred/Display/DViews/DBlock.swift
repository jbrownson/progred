import AppKit

class DBlock: NSStackView, Reconcilable {
    init(children: [D], editor: Editor) {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        children.forEach { addArrangedSubview(createView($0, editor: editor, vertical: true)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, vertical: Bool?) -> Bool {
        guard case .block(let children) = d else { return false }
        reconcileChildren(stack: self, children: children, editor: editor, vertical: true)
        return true
    }
}
