//! Ferrite's base. The engine (`mod_api`, the arismoko/horizon fork) owns items, fluids, energy,
//! capabilities and transactions; Ferrite re-exports them and adds only what the engine lacks:
//! registration tables for its own data and per-instance side configuration.

mod side;
mod table;

pub use side::{Io, SIDES, SideConfig, offset, side_index};
pub use table::{Table, TableError};

pub use mod_api;
pub use mod_api::{
    BlockSide, Capability, CapabilityKey, Direction, ENERGY, EnergyHandler, FLUIDS, FluidKey,
    FluidResource, FluidStack, Host, HostTransaction, ITEM_ENERGY, ITEM_FLUIDS, ITEMS,
    ItemCapabilityKey, ItemKey, ItemPlace, ItemResource, ItemStack, LevelRef, Overanswered,
    Registrar, RegistrationContext, ResourceHandler, ServerHost, Transaction, move_energy,
    move_resources,
};

/// Forge's `simulate = true`: runs `f` in a transaction nested in the open one, then aborts it,
/// so every write `f` made is put back. Answers what `f` answered.
pub fn simulate<T>(
    host: &mut dyn ServerHost,
    f: impl FnOnce(&mut dyn ServerHost, &Transaction) -> T,
) -> T {
    let mut answer = None;
    let _: Result<(), ()> = host.transaction(|host, tx| {
        answer = Some(f(host, tx));
        Err(())
    });
    answer.expect("the transaction runs its closure")
}
