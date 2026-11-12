//! Optional voltage tiers (HBM, IC2): a source of a higher tier than its sink overloads it.

use crate::{EnergyStorage, Fe};
use ferrite_api::{CapabilityKey, ServerHost, Table};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tier(pub u8);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierSpec {
    /// `mymod:hv`.
    pub id: String,
    /// The most FE one insert at this tier carries.
    pub max_voltage: Fe,
}

/// Tiers by number, registered by the mods that use them.
pub static TIERS: Table<Tier, TierSpec> = Table::new("ferrite:tiers");

pub fn max_voltage(tier: Tier) -> Option<Fe> {
    TIERS.get(&tier).map(|spec| spec.max_voltage)
}

/// What a sink does with energy above its tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overvoltage {
    Refuse,
    /// Takes what its own tier carries.
    Clamp,
    Explode,
}

/// [`EnergyStorage`] with tiers: the engine knows nothing of them, so only Ferrite-aware
/// moves ([`admit`]) check them.
pub trait TieredEnergy: EnergyStorage {
    fn sink_tier(&self, host: &mut dyn ServerHost) -> Tier;
    fn source_tier(&self, host: &mut dyn ServerHost) -> Tier;
    fn overvoltage(&self, _host: &mut dyn ServerHost, _tier: Tier) -> Overvoltage {
        Overvoltage::Refuse
    }
}

pub const TIERED_FE: CapabilityKey<dyn TieredEnergy> = CapabilityKey::new("ferrite:tiered_energy");

/// The outcome of offering `amount` from a `source`-tier source to a `sink`-tier sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    Pass(Fe),
    Refused,
    Explode,
}

pub fn admit(source: Tier, sink: Tier, amount: Fe, policy: Overvoltage) -> Admit {
    if source <= sink {
        return Admit::Pass(amount);
    }
    match policy {
        Overvoltage::Refuse => Admit::Refused,
        Overvoltage::Clamp => Admit::Pass(amount.min(max_voltage(sink).unwrap_or(0))),
        Overvoltage::Explode => Admit::Explode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_higher_source_tier_overloads() {
        assert_eq!(
            admit(Tier(1), Tier(1), 500, Overvoltage::Refuse),
            Admit::Pass(500)
        );
        assert_eq!(
            admit(Tier(2), Tier(1), 500, Overvoltage::Refuse),
            Admit::Refused
        );
        assert_eq!(
            admit(Tier(2), Tier(1), 500, Overvoltage::Explode),
            Admit::Explode
        );
    }
}
