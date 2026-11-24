//! The names of the users, and whose the engine's own tables are.

use std::collections::BTreeMap;

use crate::catalog::Problems;
use crate::catalog::bootstrap::SYS;
use crate::error::Result;
use crate::record::{Columns, Row, Rows};
use crate::schema::TableDefinition;
use crate::store::PageStore;

/// The catalog table of the users.
const USERS: &str = "ISYSUSER";

/// The bit of a user's type that the engine's own users have.
const ENGINE: u8 = 0x01;

/// The users of a catalog that does not say which are the engine's: those
/// with a smaller identifier are.
const FIRST_CREATED: u32 = 100;

struct User {
    id: u32,
    name: String,
    kind: Option<u8>,
}

/// Gives `tables` the names of their owners, and says which of them are the
/// engine's. What cannot be read is added to `problems`.
pub(crate) fn give(store: &PageStore, tables: &mut [TableDefinition], problems: &mut Problems) {
    let users = read(store, tables, problems);
    for table in tables {
        let owner = users.get(&table.owner_id);
        table.system = match owner.and_then(|owner| owner.kind) {
            Some(kind) => kind & ENGINE != 0,
            None => table.owner_id < FIRST_CREATED,
        };
        if let Some(owner) = owner {
            table.owner.clone_from(&owner.name);
        }
    }
}

/// Every user that can be read, by identifier. A file without a user table
/// names none.
fn read(
    store: &PageStore,
    tables: &[TableDefinition],
    problems: &mut Problems,
) -> BTreeMap<u32, User> {
    let mut users = BTreeMap::new();
    let mut catalog = tables.iter();
    let catalog = catalog.find(|table| table.owner_id == SYS && table.name == USERS);
    let Some(catalog) = catalog else {
        return users;
    };
    match Columns::with_optional(catalog, ["user_id", "user_name"], ["user_type"]) {
        Ok(columns) => {
            for row in Rows::new(store, catalog, columns) {
                match row.and_then(|row| user(&row)) {
                    Ok(user) => {
                        users.insert(user.id, user);
                    }
                    Err(error) => problems.add(error),
                }
            }
        }
        Err(error) => problems.add(error),
    }
    users
}

fn user(row: &Row) -> Result<User> {
    Ok(User {
        id: row.read(0)?,
        name: row.read(1)?,
        kind: row.read(2)?,
    })
}
