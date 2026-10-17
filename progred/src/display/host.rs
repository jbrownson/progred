//! What a world lends the toolkit's actions: the session of the editor's
//! capabilities that a dispatch acts through.

use crate::editing::Session;

pub trait Host: 'static {
    fn session(&mut self) -> Session<'_>;
}
