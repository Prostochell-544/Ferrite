//! Items. The engine owns them: `ResourceHandler<ItemResource>`, `ITEMS`, `move_resources`, all
//! in transactions. Ferrite adds what machines need over that: slot roles, per-side IO, ejecting.

use ferrite_api::{
    Capability, ITEMS, Io, ItemResource, LevelRef, Overanswered, ResourceHandler, SIDES,
    ServerHost, SideConfig, Transaction, move_resources, offset, simulate,
};

/// What a machine slot is for; automation reaches only some.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotRole {
    Input,
    Output,
    Fuel,
    Battery,
    Upgrade,
    /// Never reached from outside (blades, stamps, templates).
    Internal,
}

impl SlotRole {
    pub const fn automation_inserts(self) -> bool {
        matches!(self, SlotRole::Input | SlotRole::Fuel)
    }

    pub const fn automation_extracts(self) -> bool {
        matches!(self, SlotRole::Output)
    }
}

/// A machine's slots: their roles, and which items each takes at all.
pub struct SlotLayout {
    pub roles: &'static [SlotRole],
    pub accepts: fn(usize, &ItemResource) -> bool,
}

impl SlotLayout {
    pub fn role(&self, slot: usize) -> SlotRole {
        self.roles.get(slot).copied().unwrap_or(SlotRole::Internal)
    }
}

/// For [`SlotLayout::accepts`]: every item everywhere.
pub fn any_item(_slot: usize, _item: &ItemResource) -> bool {
    true
}

/// A machine's slots as one side reaches them: `inner` (usually the game's own, from the block
/// entity's container) narrowed by the slot roles and the side's [`Io`].
pub struct SidedSlots {
    pub inner: Box<dyn ResourceHandler<ItemResource>>,
    pub layout: &'static SlotLayout,
    pub io: Io,
}

impl SidedSlots {
    fn may_insert(&self, index: usize, resource: &ItemResource) -> bool {
        self.io.accepts()
            && self.layout.role(index).automation_inserts()
            && (self.layout.accepts)(index, resource)
    }

    fn may_extract(&self, index: usize) -> bool {
        self.io.gives() && self.layout.role(index).automation_extracts()
    }
}

impl ResourceHandler<ItemResource> for SidedSlots {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        self.inner.size(host)
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.inner.resource(host, index)
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.inner.amount(host, index)
    }
    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> u32 {
        self.inner.capacity(host, index, resource)
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> bool {
        self.may_insert(index, resource) && self.inner.is_valid(host, index, resource)
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        if !self.may_insert(index, resource) {
            return 0;
        }
        self.inner.insert(host, tx, index, resource, amount)
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        if !self.may_extract(index) {
            return 0;
        }
        self.inner.extract(host, tx, index, resource, amount)
    }
}

/// Whether all `amount` of `resource` goes into slot `slot`: tried, then put back.
pub fn fits(
    host: &mut dyn ServerHost,
    slots: &dyn ResourceHandler<ItemResource>,
    slot: usize,
    resource: &ItemResource,
    amount: u32,
) -> bool {
    simulate(host, |host, tx| {
        slots.insert(host, tx, slot, resource, amount)
    }) == amount
}

/// Moves up to `max` items out of `from`, the machine at `pos`, into the neighbours on the sides
/// `sides` gives through, in side order: how many moved.
pub fn eject(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: [i32; 3],
    sides: SideConfig,
    from: &Capability<dyn ResourceHandler<ItemResource>>,
    max: u32,
) -> Result<u32, Overanswered> {
    let mut moved = 0;
    for side in SIDES {
        if moved >= max {
            break;
        }
        if !sides.get(side).gives() {
            continue;
        }
        let Some(to) = level.capability(host, &ITEMS, offset(pos, side), Some(side.opposite()))
        else {
            continue;
        };
        moved += move_resources(host, from, &to, |_| true, max - moved, tx)?;
    }
    Ok(moved)
}
