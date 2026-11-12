use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock, PoisonError};

/// A registry of Ferrite's own data (fluid properties, energy tiers): filled while mods register,
/// frozen on the first read, then read without locks. Usable as a `static`.
pub struct Table<K, V> {
    name: &'static str,
    staged: Mutex<Option<HashMap<K, V>>>,
    frozen: OnceLock<HashMap<K, V>>,
}

/// Why a registration was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// The key is already registered in the named table.
    Duplicate(&'static str),
    /// The named table was read, so it no longer takes entries.
    Frozen(&'static str),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::Duplicate(table) => write!(f, "{table}: key already registered"),
            TableError::Frozen(table) => write!(f, "{table}: registration after the first read"),
        }
    }
}

impl std::error::Error for TableError {}

impl<K: Eq + Hash, V> Table<K, V> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            staged: Mutex::new(None),
            frozen: OnceLock::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn register(&self, key: K, value: V) -> Result<(), TableError> {
        let mut staged = self.staged.lock().unwrap_or_else(PoisonError::into_inner);
        // Checked under the lock: freezing takes it too, so no entry slips past a freeze.
        if self.frozen.get().is_some() {
            return Err(TableError::Frozen(self.name));
        }
        let map = staged.get_or_insert_with(HashMap::new);
        if map.contains_key(&key) {
            return Err(TableError::Duplicate(self.name));
        }
        map.insert(key, value);
        Ok(())
    }

    /// The entries; freezes the table.
    pub fn entries(&self) -> &HashMap<K, V> {
        self.frozen.get_or_init(|| {
            self.staged
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .unwrap_or_default()
        })
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.entries().get(key)
    }

    pub fn freeze(&self) {
        self.entries();
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_until_the_first_read() {
        let table: Table<String, i32> = Table::new("test");
        assert_eq!(table.register("a:x".into(), 1), Ok(()));
        assert_eq!(
            table.register("a:x".into(), 2),
            Err(TableError::Duplicate("test"))
        );
        assert_eq!(table.get("a:x"), Some(&1));
        assert!(table.is_frozen());
        assert_eq!(
            table.register("a:y".into(), 3),
            Err(TableError::Frozen("test"))
        );
        assert_eq!(table.get("a:y"), None);
    }

    #[test]
    fn an_empty_table_freezes_empty() {
        let table: Table<u8, u8> = Table::new("empty");
        assert!(table.entries().is_empty());
    }
}
