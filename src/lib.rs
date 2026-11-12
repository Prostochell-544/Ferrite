//! Ferrite, the mod. A mod that names `ferrite` under `[dependencies]` in its `horizon-mod.toml`
//! gets this crate under that alias: `ferrite::energy::FE`, `ferrite::recipes::RecipeBook`.

pub use ferrite_api as api;
pub use ferrite_energy as energy;
pub use ferrite_fluids as fluids;
pub use ferrite_items as items;
pub use ferrite_recipes as recipes;

use mod_api::Hooks;

#[derive(Default)]
struct Ferrite;

/// Ferrite answers no hook of its own yet: dependent mods register their machines through it.
#[mod_api::horizon_mod]
impl Hooks for Ferrite {}
