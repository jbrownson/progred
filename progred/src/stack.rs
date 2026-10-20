//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    absent, color, controls, declared, f32, f64, fidget, geometry, grap as grap_library, layout,
    list, logic, name, png, presentation, random, selection, sequence, site, text, toolpath, tree,
    u64, workspace,
};
use crate::library::{Libraries, Library};
use crate::projection::Partial;
use crate::projection::Projection;
use crate::workspace::Shown;
use std::borrow::Cow;
use std::rc::Rc;

/// The libraries Plain shows, for the editor and the tutorial page alike:
/// names, text, absent values and numbers read as themselves, and
/// everything else as Raw draws it.
pub const PLAIN: [gid::CellId; 6] = [name::ID, text::ID, absent::ID, f32::ID, f64::ID, u64::ID];

/// The sets of libraries the Projections menu offers by name. Any other set
/// is a custom one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    All,
    Plain,
    Raw,
}

impl Preset {
    pub const ALL: [Self; 3] = [Self::All, Self::Plain, Self::Raw];

    /// The libraries this preset shows.
    pub fn shown(self) -> Shown {
        match self {
            Self::All => Shown::ALL,
            Self::Plain => Shown::Only(PLAIN.to_vec()),
            Self::Raw => Shown::RAW,
        }
    }
}

#[derive(Clone)]
pub struct Stack {
    pub libraries: Libraries,
    pub projection: Projection,
    pub pane_projection: Projection,
    pub completions: crate::display::CompletionProvider,
    /// The projections and completion providers libraries bring, in load
    /// order, so an area can leave some of them off.
    partials: Rc<[(gid::CellId, Partial)]>,
    providers: Rc<[(gid::CellId, crate::display::CompletionProvider)]>,
}

impl Stack {
    /// Each library an area can switch off, one bringing a projection or
    /// offers, and its name, in load order.
    pub fn switchable(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        crate::libraries::name::library_names(&self.libraries).filter(|(library, _)| {
            self.partials.iter().any(|(id, _)| id == library)
                || self.providers.iter().any(|(id, _)| id == library)
        })
    }

    /// The libraries `shown` leaves off.
    pub fn hidden<'a>(&self, shown: &'a Shown) -> Cow<'a, [gid::CellId]> {
        match shown {
            Shown::AllBut(hidden) => Cow::Borrowed(hidden),
            Shown::Only(shown) => Cow::Owned(
                self.libraries
                    .iter()
                    .map(|(id, _)| id)
                    .filter(|id| !shown.contains(id))
                    .collect(),
            ),
        }
    }

    /// The preset showing what `shown` does of the libraries an area can
    /// switch off, if one does.
    pub fn preset(&self, shown: &Shown) -> Option<Preset> {
        Preset::ALL.into_iter().find(|preset| {
            let preset = preset.shown();
            self.switchable()
                .all(|(library, _)| preset.shows(library) == shown.shows(library))
        })
    }

    /// The completions offered with `hidden` libraries' own left off.
    pub fn completions_without(
        &self,
        hidden: &[gid::CellId],
    ) -> crate::display::CompletionProvider {
        crate::libraries::completion::combine(
            self.providers
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .map(|(_, provider)| provider.clone()),
        )
    }

    /// The document and pane projections with `hidden` libraries' own
    /// projections left off, a pane's presentation among them.
    pub fn without(&self, hidden: &[gid::CellId]) -> (Projection, Projection) {
        let document = Projection::new(
            self.partials
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .map(|(_, partial)| partial.clone()),
        );
        let pane = if hidden.contains(&presentation::ID) {
            document.clone()
        } else {
            pane(document.clone())
        };
        (document, pane)
    }
}

pub fn load() -> Stack {
    compose(BUILT_INS.iter().map(|(id, build)| (*id, build())))
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn load_selected(ids: &[gid::CellId]) -> Result<Stack, String> {
    ids.iter()
        .map(|id| {
            BUILT_INS
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, build)| (*id, build()))
                .ok_or_else(|| format!("Unknown built-in library: {id}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(compose)
}

fn compose(
    contributions: impl IntoIterator<Item = (gid::CellId, Library<crate::Editor, Hovered>)>,
) -> Stack {
    let (libraries, partials, providers) = Libraries::from_contributions(contributions);
    let providers: Rc<[_]> = providers.into();
    let completions = crate::libraries::completion::combine(
        providers.iter().map(|(_, provider)| provider.clone()),
    );
    let partials: Rc<[_]> = partials.into();
    let projection = Projection::new(partials.iter().map(|(_, partial)| partial.clone()));
    let pane_projection = pane(projection.clone());
    Stack {
        libraries,
        pane_projection,
        projection,
        completions,
        partials,
        providers,
    }
}

/// `projection` for a pane, whose entry may present its value instead.
fn pane(projection: Projection) -> Projection {
    projection.with_entry(crate::display::runtime_partial(
        presentation::projected_display,
    ))
}

type BuildLibrary = fn() -> Library<crate::Editor, Hovered>;

const BUILT_INS: &[(gid::CellId, BuildLibrary)] = &[
    (name::ID, name::library),
    (text::ID, text::library),
    (png::ID, png::library),
    (absent::ID, absent::library),
    (color::ID, color::library),
    (controls::ID, controls::library),
    (tree::ID, tree::library),
    (fidget::ID, fidget::library),
    (toolpath::ID, toolpath::library),
    (grap_library::ID, grap_library::library),
    // After Grap, which draws a number taking its operators as typing.
    (f32::ID, f32::library),
    (f64::ID, f64::library),
    (u64::ID, u64::library),
    (logic::ID, logic::library),
    (list::ID, list::library),
    (sequence::ID, sequence::library),
    (random::ID, random::library),
    (presentation::ID, presentation::library),
    (layout::ID, layout::library),
    (selection::ID, selection::library),
    (crate::libraries::path::ID, crate::libraries::path::library),
    (site::ID, site::library),
    (geometry::ID, geometry::library),
    (workspace::ID, workspace::library),
    // Tried after every built-in library.
    (declared::ID, declared::library),
];

#[cfg(test)]
mod tests;
