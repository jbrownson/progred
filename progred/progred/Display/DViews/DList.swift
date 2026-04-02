import AppKit

class DList: NSStackView, Reconcilable {
    init(elements: [D], editor: Editor) {
        super.init(frame: .zero)
        orientation = .vertical
        spacing = 0
        alignment = .leading
        translatesAutoresizingMaskIntoConstraints = false
        elements.forEach { addArrangedSubview(createView($0, editor: editor)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .list(_, let elements) = d else { return false }
        reconcileChildren(stack: self, children: elements, editor: editor)
        return true
    }
}
