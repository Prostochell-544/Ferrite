# Ferrite

An industrial API for [Horizon](https://github.com/arismoko/horizon) mods, the Rust Minecraft 26.3, written for
NTM-Oxidized and open to any mod.

The engine's mod API (`horizon-mod-api`, lib `mod_api`) already owns items, fluids, energy, capabilities and
transactions. Ferrite re-exports them and adds only what industrial mods need beyond them:

| Crate | Adds |
|---|---|
| `ferrite-api` | registration tables, per-side IO (`SideConfig`), `simulate` (a transaction that always aborts) |
| `ferrite-items` | slot roles, sided slots over the engine's `ITEMS` handlers, ejecting to neighbours |
| `ferrite-energy` | FE in 64 bits (`FE`, `ITEM_FE`), bridged to the engine's `ENERGY`; voltage tiers; the cable network |
| `ferrite-fluids` | fluid properties (temperature, gas, corrosion, radiation), containment, type-locked tanks |
| `ferrite-recipes` | machine recipes as JSON or TOML: several item and fluid inputs, outputs with chances |

The root crate `ferrite` is the mod: it re-exports the five as `ferrite::{api, items, energy, fluids, recipes}`.

## Using it

In your mod's `horizon-mod.toml`:

```toml
[dependencies]
ferrite = "^0.1"
```

The launcher gives your crate a path to Ferrite under the alias `ferrite`. A machine answers Ferrite's energy and
the engine's in one call:

```rust
ferrite::energy::provide_block_energy(r, &press, |host, at| Some(Box::new(PressEnergy(at))));
```

## Building

Pinned to Horizon `4302af451a7d72f23c4f62829a165a22a201d937` (`horizon-mod.toml`, `[native]`). The launcher supplies
`horizon-mod-api`; generate the registry lock once with `horizon-launcher native-lock .`, then
`horizon-launcher native-build . --instance <instance> --trust-native-code`.
