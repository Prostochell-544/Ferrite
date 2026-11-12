//! Ferrite energy (FE). One FE is one unit of the engine's `ENERGY`, held in an `i64` where the
//! engine's `EnergyHandler` holds a `u32`. Writes happen in the engine's transactions: to simulate,
//! use [`ferrite_api::simulate`]. A machine answers [`FE`] and, through [`provide_block_energy`],
//! the engine's `ENERGY` as well, so other mods' cables reach it.

pub mod net;
mod tier;

pub use net::{Connection, EnergyHandlers, NetId, NodeId, Nodespace, Port, Priority, around};
pub use tier::{
    Admit, Overvoltage, TIERED_FE, TIERS, Tier, TierSpec, TieredEnergy, admit, max_voltage,
};

use ferrite_api::mod_api::BlockEntityTypeKey;
use ferrite_api::{
    BlockSide, Capability, CapabilityKey, ENERGY, EnergyHandler, HostTransaction,
    ItemCapabilityKey, Registrar, ServerHost, Transaction,
};
use std::sync::Arc;

pub type Fe = i64;

/// A block's or an item's FE: the engine's `EnergyHandler` in 64 bits. Stateless and given the
/// host, as engine handlers are: its amount lives in the block entity's data or the stack.
pub trait EnergyStorage: Send + Sync {
    fn stored(&self, host: &mut dyn ServerHost) -> Fe;
    fn capacity(&self, host: &mut dyn ServerHost) -> Fe;
    /// Puts up to `amount` in, in `tx`: how much went. Never more than `amount`.
    fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: Fe) -> Fe;
    /// Takes up to `amount` out, in `tx`: how much came. Never more than `amount`.
    fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: Fe) -> Fe;
    /// The network serves higher priorities first.
    fn priority(&self, _host: &mut dyn ServerHost) -> Priority {
        Priority::Normal
    }
}

pub const FE: CapabilityKey<dyn EnergyStorage> = CapabilityKey::new("ferrite:energy");
pub const ITEM_FE: ItemCapabilityKey<dyn EnergyStorage> = ItemCapabilityKey::new("ferrite:energy");

/// A handler answered more than it was asked: a mod bug. The move it was part of is undone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeOveranswered {
    /// The mod whose handler answered, or "the game".
    pub owner: String,
}

/// Moves up to `max` FE out of `from` into `to`, in `tx`, as the engine's `move_energy`: all or
/// nothing in a transaction of its own, a move `to` takes only part of tried again with that part.
pub fn move_fe(
    host: &mut dyn ServerHost,
    from: &Capability<dyn EnergyStorage>,
    to: &Capability<dyn EnergyStorage>,
    max: Fe,
    tx: &Transaction,
) -> Result<Fe, FeOveranswered> {
    // The attempts below nest in `tx`, the open transaction.
    let _ = tx;
    let want = max.min(from.stored(host));
    if want <= 0 {
        return Ok(0);
    }
    match attempt(host, from, to, want) {
        Attempt::Moved(moved) => Ok(moved),
        Attempt::Partial(part) if part > 0 => match attempt(host, from, to, part) {
            Attempt::Moved(moved) => Ok(moved),
            Attempt::Partial(_) => Ok(0),
            Attempt::Over(owner) => Err(FeOveranswered { owner }),
        },
        Attempt::Partial(_) => Ok(0),
        Attempt::Over(owner) => Err(FeOveranswered { owner }),
    }
}

enum Attempt {
    Moved(Fe),
    Partial(Fe),
    Over(String),
}

fn attempt(
    host: &mut dyn ServerHost,
    from: &Capability<dyn EnergyStorage>,
    to: &Capability<dyn EnergyStorage>,
    want: Fe,
) -> Attempt {
    let moved = host.transaction(|host, tx| {
        let got = from.extract(host, tx, want);
        if !(0..=want).contains(&got) {
            return Err(Attempt::Over(from.owner()));
        }
        let went = to.insert(host, tx, got);
        if !(0..=got).contains(&went) {
            return Err(Attempt::Over(to.owner()));
        }
        if went < got {
            return Err(Attempt::Partial(went));
        }
        Ok(went)
    });
    match moved {
        Ok(moved) => Attempt::Moved(moved),
        Err(attempt) => attempt,
    }
}

/// An [`EnergyStorage`] as the engine's `EnergyHandler`, clamped to `u32`.
pub struct AsEngineEnergy(pub Box<dyn EnergyStorage>);

fn clamp(fe: Fe) -> u32 {
    fe.clamp(0, u32::MAX as Fe) as u32
}

impl EnergyHandler for AsEngineEnergy {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        clamp(self.0.stored(host))
    }
    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        clamp(self.0.capacity(host))
    }
    fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        clamp(self.0.insert(host, tx, amount as Fe)).min(amount)
    }
    fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        clamp(self.0.extract(host, tx, amount as Fe)).min(amount)
    }
}

/// Answers [`FE`] and the engine's `ENERGY` for block entity type `kind` from one provider.
pub fn provide_block_energy(
    r: &mut dyn Registrar,
    kind: &BlockEntityTypeKey,
    provider: impl Fn(&mut dyn ServerHost, BlockSide) -> Option<Box<dyn EnergyStorage>>
    + Send
    + Sync
    + 'static,
) {
    let provider = Arc::new(provider);
    let engine = Arc::clone(&provider);
    r.block_entity_capability(&FE, kind, move |host, at| provider(host, at));
    r.block_entity_capability(&ENERGY, kind, move |host, at| {
        engine(host, at).map(|storage| Box::new(AsEngineEnergy(storage)) as Box<dyn EnergyHandler>)
    });
}
