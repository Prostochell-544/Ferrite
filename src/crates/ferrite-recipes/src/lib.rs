//! Machine recipes as data: several item and fluid inputs, outputs with chances, energy and time.
//! Read from recipe files through the engine (`recipes_of_type`, so data packs and `/reload`
//! apply), or from JSON or TOML a mod carries.
//!
//! ```json
//! { "type": "mymod:assembler", "duration": 200, "energy": 100,
//!   "items": [{ "item": "#c:ingots/steel", "count": 2 }],
//!   "fluids": [{ "fluid": "mymod:sulfuric_acid", "amount": 500 }],
//!   "outputs": [{ "item": "mymod:plate", "count": 1 }, { "item": "mymod:scrap", "chance": 0.1 }] }
//! ```

use ferrite_api::mod_api::Identifier;
use ferrite_api::{FluidKey, FluidStack, Host, ItemKey, ItemStack};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    /// `mymod:assembler/plate`; from the file's place for a recipe the engine read.
    #[serde(default)]
    pub id: String,
    /// The machine's recipe type, `mymod:assembler`: a recipe file's `type`.
    #[serde(alias = "type")]
    pub machine: String,
    #[serde(default)]
    pub items: Vec<ItemIn>,
    #[serde(default)]
    pub fluids: Vec<FluidIn>,
    #[serde(default)]
    pub outputs: Vec<Output>,
    /// Ticks.
    #[serde(default)]
    pub duration: u32,
    /// FE per tick.
    #[serde(default)]
    pub energy: i64,
    /// Whatever else a machine reads (heat, tier, a template).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemIn {
    /// `ns:item`, or `#ns:tag`.
    pub item: String,
    #[serde(default = "one")]
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FluidIn {
    pub fluid: String,
    /// Millibuckets.
    pub amount: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Output {
    Item {
        item: String,
        #[serde(default = "one")]
        count: u32,
        #[serde(default = "always")]
        chance: f32,
    },
    Fluid {
        fluid: String,
        amount: u32,
        #[serde(default = "always")]
        chance: f32,
    },
}

impl Output {
    pub fn chance(&self) -> f32 {
        match self {
            Output::Item { chance, .. } | Output::Fluid { chance, .. } => *chance,
        }
    }
}

fn one() -> u32 {
    1
}

fn always() -> f32 {
    1.0
}

#[derive(Debug)]
pub enum RecipeError {
    Json(serde_json::Error),
    Toml(toml::de::Error),
    Invalid { id: String, reason: &'static str },
    Duplicate(String),
}

impl fmt::Display for RecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecipeError::Json(e) => write!(f, "recipe JSON: {e}"),
            RecipeError::Toml(e) => write!(f, "recipe TOML: {e}"),
            RecipeError::Invalid { id, reason } => write!(f, "recipe {id}: {reason}"),
            RecipeError::Duplicate(id) => write!(f, "recipe {id}: already loaded"),
        }
    }
}

impl std::error::Error for RecipeError {}

impl From<serde_json::Error> for RecipeError {
    fn from(e: serde_json::Error) -> Self {
        RecipeError::Json(e)
    }
}

impl From<toml::de::Error> for RecipeError {
    fn from(e: toml::de::Error) -> Self {
        RecipeError::Toml(e)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    Many(Vec<Recipe>),
    One(Box<Recipe>),
}

#[derive(Deserialize)]
struct TomlFile {
    #[serde(default)]
    recipe: Vec<Recipe>,
}

/// Loaded recipes, by machine. Reload by clearing and loading again (`server_data_loaded`).
#[derive(Clone, Debug, Default)]
pub struct RecipeBook {
    by_machine: HashMap<String, Vec<Recipe>>,
    ids: HashSet<String>,
}

impl RecipeBook {
    pub fn new() -> RecipeBook {
        RecipeBook::default()
    }

    pub fn clear(&mut self) {
        self.by_machine.clear();
        self.ids.clear();
    }

    pub fn add(&mut self, recipe: Recipe) -> Result<(), RecipeError> {
        validate(&recipe)?;
        if !self.ids.insert(recipe.id.clone()) {
            return Err(RecipeError::Duplicate(recipe.id));
        }
        self.by_machine
            .entry(recipe.machine.clone())
            .or_default()
            .push(recipe);
        Ok(())
    }

    /// One recipe, or an array of them.
    pub fn load_json(&mut self, src: &str) -> Result<usize, RecipeError> {
        let recipes = match serde_json::from_str(src)? {
            OneOrMany::Many(recipes) => recipes,
            OneOrMany::One(recipe) => vec![*recipe],
        };
        self.add_all(recipes)
    }

    /// `[[recipe]]` tables.
    pub fn load_toml(&mut self, src: &str) -> Result<usize, RecipeError> {
        let file: TomlFile = toml::from_str(src)?;
        self.add_all(file.recipe)
    }

    /// The recipe files of type `machine` in force: the mods' and the world's data packs'.
    pub fn load_engine(&mut self, host: &dyn Host, machine: &str) -> Result<usize, RecipeError> {
        let found = host.recipes_of_type(machine);
        let mut recipes = Vec::with_capacity(found.len());
        for file in found.iter() {
            let mut recipe: Recipe = serde_json::from_value((*file.json).clone())?;
            recipe.id = file.id.clone();
            recipe.machine = file.kind.clone();
            recipes.push(recipe);
        }
        self.add_all(recipes)
    }

    fn add_all(&mut self, recipes: Vec<Recipe>) -> Result<usize, RecipeError> {
        let n = recipes.len();
        for recipe in recipes {
            self.add(recipe)?;
        }
        Ok(n)
    }

    pub fn recipes(&self, machine: &str) -> &[Recipe] {
        self.by_machine.get(machine).map_or(&[], Vec::as_slice)
    }

    /// The first recipe of `machine`, in load order, whose every input has a slot or tank of its
    /// own holding enough. Items and fluids no input takes don't stop a match.
    pub fn find_with<I, F>(
        &self,
        machine: &str,
        items: &[Option<(I, u32)>],
        fluids: &[Option<(F, u32)>],
        item_matches: impl Fn(&I, &ItemIn) -> bool,
        fluid_matches: impl Fn(&F, &FluidIn) -> bool,
    ) -> Option<&Recipe> {
        self.recipes(machine).iter().find(|recipe| {
            assign(
                &recipe.items,
                items,
                &mut vec![false; items.len()],
                &|have, want| have.1 >= want.count && item_matches(&have.0, want),
            ) && assign(
                &recipe.fluids,
                fluids,
                &mut vec![false; fluids.len()],
                &|have, want| have.1 >= want.amount && fluid_matches(&have.0, want),
            )
        })
    }

    /// [`find_with`](Self::find_with) on a machine's slots and tanks, ids and tags read through
    /// the engine.
    pub fn find(
        &self,
        host: &dyn Host,
        machine: &str,
        items: &[Option<ItemStack>],
        fluids: &[Option<FluidStack>],
    ) -> Option<&Recipe> {
        let items: Vec<Option<(ItemKey, u32)>> = items
            .iter()
            .map(|stack| {
                stack
                    .as_ref()
                    .filter(|stack| stack.count() > 0)
                    .map(|stack| (stack.item(), stack.count()))
            })
            .collect();
        let fluids: Vec<Option<(FluidKey, u32)>> = fluids
            .iter()
            .map(|stack| {
                let stack = stack.as_ref().filter(|stack| stack.amount > 0)?;
                Some((stack.fluid.key()?, stack.amount))
            })
            .collect();
        self.find_with(
            machine,
            &items,
            &fluids,
            |&item, want| match want.item.strip_prefix('#') {
                Some(tag) => item_in_tag(host, item, tag),
                None => item.id(host) == want.item,
            },
            |&fluid, want| fluid.id(host) == want.fluid,
        )
    }
}

fn item_in_tag(host: &dyn Host, item: ItemKey, tag: &str) -> bool {
    let Ok(id) = Identifier::parse(tag) else {
        return false;
    };
    let tags = host.tags();
    tags.key(&id)
        .and_then(|tag| tags.is_in_tag(item, tag))
        .unwrap_or(false)
}

/// Whether each of `wants` gets a slot of `haves` of its own that `fits`, trying every
/// assignment: a tag input must not take the only slot an exact input fits.
fn assign<H, W>(
    wants: &[W],
    haves: &[Option<H>],
    used: &mut Vec<bool>,
    fits: &impl Fn(&H, &W) -> bool,
) -> bool {
    let Some((want, rest)) = wants.split_first() else {
        return true;
    };
    for (i, have) in haves.iter().enumerate() {
        let Some(have) = have else { continue };
        if used[i] || !fits(have, want) {
            continue;
        }
        used[i] = true;
        if assign(rest, haves, used, fits) {
            return true;
        }
        used[i] = false;
    }
    false
}

/// The outputs a run of `recipe` makes: each by its chance, drawn from `rand` in `[0, 1)` (the
/// level's random, for vanilla's order).
pub fn roll(recipe: &Recipe, mut rand: impl FnMut() -> f32) -> Vec<&Output> {
    recipe
        .outputs
        .iter()
        .filter(|output| output.chance() >= 1.0 || rand() < output.chance())
        .collect()
}

fn validate(recipe: &Recipe) -> Result<(), RecipeError> {
    let invalid = |reason| {
        Err(RecipeError::Invalid {
            id: recipe.id.clone(),
            reason,
        })
    };
    if !recipe.id.contains(':') {
        return invalid("id needs a namespace");
    }
    if !recipe.machine.contains(':') {
        return invalid("machine needs a namespace");
    }
    if recipe.items.iter().any(|input| input.count == 0) {
        return invalid("an item input of count 0");
    }
    if recipe.fluids.iter().any(|input| input.amount == 0) {
        return invalid("a fluid input of 0 mB");
    }
    for output in &recipe.outputs {
        let chance = output.chance();
        if !(chance > 0.0 && chance <= 1.0) {
            return invalid("an output chance outside (0, 1]");
        }
        match output {
            Output::Item { count: 0, .. } => return invalid("an item output of count 0"),
            Output::Fluid { amount: 0, .. } => return invalid("a fluid output of 0 mB"),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLATE: &str = r##"{
        "id": "t:plate", "type": "t:press", "duration": 200, "energy": 100,
        "items": [{ "item": "#c:ingots", "count": 2 }, { "item": "t:steel_ingot" }],
        "fluids": [{ "fluid": "t:acid", "amount": 500 }],
        "outputs": [{ "item": "t:plate" }, { "fluid": "t:slag", "amount": 50, "chance": 0.25 }]
    }"##;

    fn book() -> RecipeBook {
        let mut book = RecipeBook::new();
        assert_eq!(book.load_json(PLATE).unwrap(), 1);
        book
    }

    fn find<'b>(
        book: &'b RecipeBook,
        items: &[Option<(&str, u32)>],
        fluids: &[Option<(&str, u32)>],
    ) -> Option<&'b Recipe> {
        let ingots = ["t:steel_ingot", "t:iron_ingot"];
        book.find_with(
            "t:press",
            items,
            fluids,
            |&item, want| match want.item.strip_prefix('#') {
                Some("c:ingots") => ingots.contains(&item),
                Some(_) => false,
                None => item == want.item,
            },
            |&fluid, want| fluid == want.fluid,
        )
    }

    #[test]
    fn reads_json_with_defaults() {
        let book = book();
        let recipe = &book.recipes("t:press")[0];
        assert_eq!(recipe.items[1].count, 1);
        assert_eq!(recipe.outputs[0].chance(), 1.0);
        assert_eq!(
            recipe.outputs[1],
            Output::Fluid {
                fluid: "t:slag".into(),
                amount: 50,
                chance: 0.25
            }
        );
    }

    #[test]
    fn reads_toml() {
        let mut book = RecipeBook::new();
        let src = r#"
            [[recipe]]
            id = "t:dust"
            machine = "t:shredder"
            duration = 60
            items = [{ item = "t:ore" }]
            outputs = [{ item = "t:dust", count = 2 }]
        "#;
        assert_eq!(book.load_toml(src).unwrap(), 1);
        assert_eq!(
            book.recipes("t:shredder")[0].outputs[0],
            Output::Item {
                item: "t:dust".into(),
                count: 2,
                chance: 1.0
            }
        );
    }

    #[test]
    fn a_tag_input_leaves_the_exact_one_its_slot() {
        let book = book();
        // The tag could take the steel; it must take the iron so the steel input has a slot.
        let items = [Some(("t:steel_ingot", 1)), Some(("t:iron_ingot", 2)), None];
        assert!(find(&book, &items, &[Some(("t:acid", 1000))]).is_some());
        assert!(find(&book, &items, &[Some(("t:acid", 499))]).is_none());
        assert!(find(&book, &items[..1], &[Some(("t:acid", 1000))]).is_none());
    }

    #[test]
    fn refuses_bad_and_duplicate_recipes() {
        let mut book = book();
        assert!(matches!(
            book.load_json(PLATE),
            Err(RecipeError::Duplicate(_))
        ));
        let bad = PLATE
            .replace("\"chance\": 0.25", "\"chance\": 1.5")
            .replace("t:plate\"", "t:plate2\"");
        assert!(matches!(
            book.load_json(&bad),
            Err(RecipeError::Invalid { .. })
        ));
    }

    #[test]
    fn rolls_each_output_by_its_chance() {
        let book = book();
        let recipe = &book.recipes("t:press")[0];
        assert_eq!(roll(recipe, || 0.9).len(), 1);
        assert_eq!(roll(recipe, || 0.1).len(), 2);
    }
}
