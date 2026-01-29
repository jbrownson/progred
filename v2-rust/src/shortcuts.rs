use eframe::egui::{Key, KeyboardShortcut, Modifiers};

pub const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
pub const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
pub const INSERT_NODE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::N);
pub const DELETE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Backspace);

pub fn format(shortcut: &KeyboardShortcut) -> String {
    shortcut.format(&eframe::egui::ModifierNames::NAMES, true)
}
