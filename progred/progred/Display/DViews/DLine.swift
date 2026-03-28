import AppKit

class DLine: NSStackView, Reconcilable {
    init(children: [D], editor: Editor, parentReadOnly: Bool) {
        super.init(frame: .zero)
        orientation = .horizontal
        alignment = .top
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        children.forEach { addArrangedSubview(createView($0, editor: editor, parentReadOnly: parentReadOnly)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?, inCycle: Bool) -> Bool {
        guard case .line(let children) = d else { return false }
        reconcileChildren(stack: self, children: children, editor: editor, parentReadOnly: parentReadOnly)
        return true
    }
}
