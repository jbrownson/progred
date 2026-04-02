import AppKit

class DList: NSStackView, Reconcilable {
    init(elements: [D], editor: Editor) {
        super.init(frame: .zero)
        orientation = .vertical
        spacing = 0
        alignment = .leading
        translatesAutoresizingMaskIntoConstraints = false
        elements.forEach { addArrangedSubview(createView($0, editor: editor, vertical: true)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?, vertical: Bool?) -> Bool {
        guard case .list(_, let elements) = d else { return false }
        reconcileChildren(stack: self, children: elements, editor: editor, vertical: true)
        return true
    }
}
