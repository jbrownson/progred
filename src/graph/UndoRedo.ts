export class UndoRedo {
  constructor(
    public readonly undo: () => void,
    public readonly redo: () => void,
    public readonly selectionAction: boolean) {} }