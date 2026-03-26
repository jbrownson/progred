import AppKit

class DList: NSStackView, Reconcilable {
    var parentReadOnly: Bool

    init(elements: [D], editor: Editor, parentReadOnly: Bool) {
        self.parentReadOnly = parentReadOnly
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        elements.forEach { addArrangedSubview(createView($0, editor: editor, parentReadOnly: parentReadOnly)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .list(_, let elements) = d else { return false }
        reconcileChildren(stack: self, children: elements, editor: editor, parentReadOnly: parentReadOnly)
        return true
    }
}
