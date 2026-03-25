import AppKit

class DList: NSStackView, DView {
    init(elements: [D], editor: Editor) {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 0
        translatesAutoresizingMaskIntoConstraints = false
        elements.forEach { addArrangedSubview(createView($0, editor: editor)) }
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor) -> Bool {
        guard case .list(_, let elements) = d else { return false }
        reconcileChildren(stack: self, children: elements, editor: editor)
        return true
    }
}
