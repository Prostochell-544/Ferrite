//! Fluids. The engine owns them: `FluidStack`, `FluidResource`, `FLUIDS`, `ITEM_FLUIDS`, mod fluid
//! declarations, all in millibuckets and transactions. Ferrite adds the properties industrial mods
//! read (temperature, gas, corrosion, radiation) and tanks that refuse what they can't hold.

use ferrite_api::{FluidKey, FluidResource, ResourceHandler, ServerHost, Table, Transaction};
use serde::{Deserialize, Serialize};

/// What a fluid is like, beyond what the engine declares.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FluidTraits {
    /// °C; room temperature is 20.
    #[serde(default = "room")]
    pub temperature: i32,
    #[serde(default)]
    pub is_gas: bool,
    /// 0 none, 1 corrosive, 2 strongly corrosive.
    #[serde(default)]
    pub corrosivity: u8,
    /// Radiation per 1000 mB.
    #[serde(default)]
    pub radiation: f32,
    /// 0xRRGGBB, for gauges.
    #[serde(default = "white")]
    pub color: u32,
}

fn room() -> i32 {
    20
}

fn white() -> u32 {
    0xFF_FF_FF
}

impl Default for FluidTraits {
    fn default() -> Self {
        FluidTraits {
            temperature: room(),
            is_gas: false,
            corrosivity: 0,
            radiation: 0.0,
            color: white(),
        }
    }
}

/// By fluid id (`mymod:hydrogen`): fluid keys are given out only once the fluids are registered.
pub static FLUID_TRAITS: Table<String, FluidTraits> = Table::new("ferrite:fluid_traits");

pub fn traits_of_key(host: &dyn ServerHost, key: FluidKey) -> Option<&'static FluidTraits> {
    FLUID_TRAITS.get(key.id(host).as_str())
}

/// None for a fluid with no traits, or one whose mod is gone.
pub fn traits_of(host: &dyn ServerHost, fluid: &FluidResource) -> Option<&'static FluidTraits> {
    traits_of_key(host, fluid.fluid().key()?)
}

/// What a tank, pipe or cell survives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Containment {
    pub max_temperature: i32,
    pub max_corrosivity: u8,
    pub gas_tight: bool,
}

impl Containment {
    pub const ANY: Containment = Containment {
        max_temperature: i32::MAX,
        max_corrosivity: u8::MAX,
        gas_tight: true,
    };

    pub fn holds(&self, traits: &FluidTraits) -> bool {
        traits.temperature <= self.max_temperature
            && traits.corrosivity <= self.max_corrosivity
            && (self.gas_tight || !traits.is_gas)
    }
}

/// A tank over `inner` (usually the game's own tanks) that takes only `locked`, when set, and only
/// what `containment` holds. A fluid without traits counts as room-temperature, inert liquid.
pub struct TypedTank {
    pub inner: Box<dyn ResourceHandler<FluidResource>>,
    pub locked: Option<FluidKey>,
    pub containment: Containment,
}

impl TypedTank {
    fn may_insert(&self, host: &mut dyn ServerHost, resource: &FluidResource) -> bool {
        let key = resource.fluid().key();
        if self.locked.is_some() && key != self.locked {
            return false;
        }
        let traits = key.and_then(|key| traits_of_key(host, key));
        self.containment
            .holds(traits.unwrap_or(&FluidTraits::default()))
    }
}

impl ResourceHandler<FluidResource> for TypedTank {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        self.inner.size(host)
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<FluidResource> {
        self.inner.resource(host, index)
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.inner.amount(host, index)
    }
    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &FluidResource) -> u32 {
        self.inner.capacity(host, index, resource)
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &FluidResource) -> bool {
        self.may_insert(host, resource) && self.inner.is_valid(host, index, resource)
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &FluidResource,
        amount: u32,
    ) -> u32 {
        if !self.may_insert(host, resource) {
            return 0;
        }
        self.inner.insert(host, tx, index, resource, amount)
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &FluidResource,
        amount: u32,
    ) -> u32 {
        self.inner.extract(host, tx, index, resource, amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containment_checks_heat_corrosion_and_gas() {
        let steel = Containment {
            max_temperature: 1500,
            max_corrosivity: 0,
            gas_tight: true,
        };
        let acid = FluidTraits {
            corrosivity: 2,
            ..FluidTraits::default()
        };
        let steam = FluidTraits {
            temperature: 100,
            is_gas: true,
            ..FluidTraits::default()
        };
        assert!(!steel.holds(&acid));
        assert!(steel.holds(&steam));
        assert!(
            !Containment {
                gas_tight: false,
                ..steel
            }
            .holds(&steam)
        );
    }
}
