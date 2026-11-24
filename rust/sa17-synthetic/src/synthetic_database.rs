//! A whole synthetic file: the tables, the catalog that describes them, and
//! the key that scrambles every page.

use crate::cell::Cell;
use crate::keys;
use crate::pages::Pages;
use crate::scramble::{self, Key, Sign};
use crate::synthetic_table::{OWNER, SyntheticColumn, SyntheticRow, SyntheticTable, Written};

const PAGE: usize = 4096;

/// The page that lists the pages of the table catalog.
const TABLE_PAGE_LIST: u32 = 10;

const TABLE_CATALOG: u32 = 1;
const COLUMN_CATALOG: u32 = 2;
const INDEX_CATALOG: u32 = 3;
const INDEX_COLUMN_CATALOG: u32 = 5;
const FOREIGN_KEY_CATALOG: u32 = 6;
const USER_CATALOG: u32 = 9;

/// Identifiers of the tables a test adds start here.
const FIRST_TABLE: u32 = 1000;

/// The engine's own user, which owns the catalog.
const SYSTEM_USER: u32 = 0;

/// The types of a user: the engine's own role, another of its users, one that
/// a person created, and one of those that stands for a group.
const ENGINE_ROLE: u8 = 1;
const ENGINE_USER: u8 = 5;
const CREATED_USER: u8 = 12;
const CREATED_GROUP: u8 = 14;

/// A page list has a bit for each page up to its last one, rounded up to this
/// many of them.
const PAGE_LIST_BITS: u32 = 2048;

/// A synthetic database file in the making.
#[derive(Clone, Debug)]
pub struct SyntheticDatabase {
    key: Key,
    owner: String,
    users: Vec<User>,
    user_types: bool,
    tables: Vec<SyntheticTable>,
    patches: Vec<Patch>,
    checkpoint_copy: bool,
}

/// A user of the database.
#[derive(Clone, Debug)]
struct User {
    id: u32,
    name: String,
    kind: u8,
}

/// Bytes to put on a page before it is scrambled.
#[derive(Clone, Debug)]
struct Patch {
    page: u32,
    offset: usize,
    bytes: Vec<u8>,
}

/// A table as the catalog will describe it.
struct Entry<'table> {
    id: u32,
    table: &'table SyntheticTable,
    pages: Option<Vec<u32>>,
    extension_pages: Vec<u32>,
    rows: u64,
}

impl<'table> Entry<'table> {
    fn new(id: u32, table: &'table SyntheticTable, written: Written) -> Self {
        Self {
            id,
            table,
            pages: table.lists_pages().then_some(written.pages),
            extension_pages: written.extension_pages,
            rows: written.live_rows,
        }
    }
}

impl Default for SyntheticDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticDatabase {
    /// A file with only the catalog, scrambled with a positive key.
    #[must_use]
    pub fn new() -> Self {
        Self {
            key: Key {
                mask: 0x0000_1234,
                addend: 0x0123_4567,
                sign: Sign::Positive,
            },
            owner: "owner".to_owned(),
            users: Vec::new(),
            user_types: true,
            tables: Vec::new(),
            patches: Vec::new(),
            checkpoint_copy: false,
        }
    }

    /// Sets the constants and the sign of the page keys.
    #[must_use]
    pub fn key(mut self, mask: u32, addend: u32, sign: Sign) -> Self {
        self.key = Key { mask, addend, sign };
        self
    }

    /// Names the user that owns every table which does not say otherwise.
    #[must_use]
    pub fn owner(mut self, name: &str) -> Self {
        name.clone_into(&mut self.owner);
        self
    }

    /// Adds a user that a person created, who can own tables.
    #[must_use]
    pub fn user(self, id: u32, name: &str) -> Self {
        self.user_of(id, name, CREATED_USER)
    }

    /// Adds one of the engine's own users, who can own tables.
    #[must_use]
    pub fn engine_user(self, id: u32, name: &str) -> Self {
        self.user_of(id, name, ENGINE_USER)
    }

    fn user_of(mut self, id: u32, name: &str, kind: u8) -> Self {
        let name = name.to_owned();
        self.users.push(User { id, name, kind });
        self
    }

    /// Leaves out the catalog's column with the type of each user.
    #[must_use]
    pub fn without_user_types(mut self) -> Self {
        self.user_types = false;
        self
    }

    /// Adds a table. Tables get the identifiers 1000, 1001 and so on.
    #[must_use]
    pub fn table(mut self, table: SyntheticTable) -> Self {
        self.tables.push(table);
        self
    }

    /// Puts `bytes` at `offset` of the readable page `page`, after everything
    /// else is laid out. The page is scrambled and gets its checksum as usual,
    /// so this is a file that only its contents show to be damaged.
    #[must_use]
    pub fn patch(mut self, page: u32, offset: usize, bytes: &[u8]) -> Self {
        self.patches.push(Patch {
            page,
            offset,
            bytes: bytes.to_vec(),
        });
        self
    }

    /// Appends a copy of the first table's first page past the end of the
    /// database, the way a checkpoint leaves old pages behind.
    #[must_use]
    pub fn checkpoint_copy(mut self) -> Self {
        self.checkpoint_copy = true;
        self
    }

    /// Lays out every page, scrambles them and returns the file's bytes.
    ///
    /// # Panics
    ///
    /// Panics when a row does not match its table's columns, when a key or an
    /// index names a column or a table that is not there, and when a patch is
    /// outside the file or its page.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let mut pages = Pages::new();
        let table_catalog = catalog("ISYSTAB", table_catalog_columns());
        let column_catalog = catalog("ISYSTABCOL", column_catalog_columns());
        let user_catalog = self.user_catalog();
        let keys = keys::catalogs(FIRST_TABLE, &self.tables);
        let key_catalogs = [
            (INDEX_CATALOG, &keys.indexes),
            (INDEX_COLUMN_CATALOG, &keys.columns),
            (FOREIGN_KEY_CATALOG, &keys.foreign_keys),
            (USER_CATALOG, &user_catalog),
        ];

        let mut entries = Vec::new();
        for (id, table) in (FIRST_TABLE..).zip(&self.tables) {
            let written = table.write(id, &mut pages);
            entries.push(Entry::new(id, table, written));
        }
        let first_table_page = entries
            .first()
            .and_then(|entry| entry.pages.as_ref())
            .and_then(|pages| pages.first().copied());

        for (id, table) in key_catalogs {
            let written = table.write(id, &mut pages);
            entries.push(Entry::new(id, table, written));
        }

        // The column catalog describes itself and the table catalog too.
        let mut described = column_catalog.clone();
        let every_table = entries.iter().map(|entry| (entry.id, entry.table)).chain([
            (TABLE_CATALOG, &table_catalog),
            (COLUMN_CATALOG, &column_catalog),
        ]);
        for (id, table) in every_table {
            for (number, column) in (1..).zip(&table.columns) {
                described = described.row(column_row(id, number, column));
            }
        }
        let written = described.write(COLUMN_CATALOG, &mut pages);
        entries.push(Entry::new(COLUMN_CATALOG, &column_catalog, written));

        // The table catalog cannot list its own pages. Page 10 has those of its
        // rows, and nothing has the others.
        let own_rows = entries.len() as u64 + 1;
        entries.push(Entry {
            id: TABLE_CATALOG,
            table: &table_catalog,
            pages: None,
            extension_pages: Vec::new(),
            rows: own_rows,
        });
        let mut listed = table_catalog.clone();
        for entry in &entries {
            listed = listed.row(table_row(entry));
        }
        let written = listed.write(TABLE_CATALOG, &mut pages);
        write_page_list(pages.page_mut(TABLE_PAGE_LIST), &written.pages);

        let database_pages = pages.len();
        if self.checkpoint_copy
            && let Some(page) = first_table_page
        {
            pages.add_copy(page);
        }
        write_superblock(pages.page_mut(0), database_pages);
        for patch in &self.patches {
            let end = patch.offset + patch.bytes.len();
            pages.page_mut(patch.page)[patch.offset..end].copy_from_slice(&patch.bytes);
        }
        scramble::stamp(pages.page_mut(0));

        let mut file = Vec::with_capacity(pages.len() as usize * PAGE);
        for (number, page) in (0..).zip(pages.into_pages()) {
            let never_written = page.iter().all(|&byte| byte == 0);
            if number == 0 || never_written {
                file.extend_from_slice(&page);
            } else {
                file.extend_from_slice(&self.key.scramble(number, &page));
            }
        }
        file
    }
}

impl SyntheticDatabase {
    /// The catalog table of the users, with its rows.
    fn user_catalog(&self) -> SyntheticTable {
        let mut columns = vec![
            required("user_id", 21, 4),
            required("object_id", 23, 8),
            required("user_name", 8, 128),
        ];
        if self.user_types {
            columns.push(required("user_type", 19, 1));
        }
        let mut users = vec![
            User {
                id: SYSTEM_USER,
                name: "SYS".to_owned(),
                kind: ENGINE_ROLE,
            },
            User {
                id: OWNER,
                name: self.owner.clone(),
                kind: CREATED_GROUP,
            },
        ];
        users.extend_from_slice(&self.users);
        let mut table = catalog("ISYSUSER", columns);
        for user in users {
            let kind = self.user_types.then_some(Cell::tiny_int(user.kind));
            let cells = [
                Cell::unsigned_int(user.id),
                Cell::unsigned_big_int(user.id.into()),
                Cell::text(&user.name),
            ];
            table = table.row(SyntheticRow::new(cells.into_iter().chain(kind)));
        }
        table
    }
}

/// A catalog table without rows.
pub(crate) fn catalog(name: &str, columns: Vec<SyntheticColumn>) -> SyntheticTable {
    let mut table = SyntheticTable::new(name).owned_by(SYSTEM_USER);
    for column in columns {
        table = table.column(column);
    }
    table
}

/// A column that always holds a value.
pub(crate) fn required(name: &str, domain: u16, width: u32) -> SyntheticColumn {
    SyntheticColumn::new(name, domain).width(width)
}

/// A column that may hold `NULL`.
pub(crate) fn optional(name: &str, domain: u16, width: u32) -> SyntheticColumn {
    required(name, domain, width).nullable()
}

fn table_catalog_columns() -> Vec<SyntheticColumn> {
    vec![
        required("table_id", 21, 4),
        required("dbspace_id", 1, 2),
        required("count", 23, 8),
        required("creator", 21, 4),
        required("table_page_count", 2, 4),
        required("ext_page_count", 2, 4),
        required("commit_action", 2, 4),
        required("share_type", 2, 4),
        required("object_id", 23, 8),
        required("last_modified_at", 13, 8),
        required("table_name", 8, 128),
        required("table_type", 19, 1),
        required("replicate", 8, 1),
        required("server_type", 19, 1),
        optional("tab_page_list", 31, 32767),
        optional("ext_page_list", 31, 32767),
        optional("pct_free", 21, 4),
        optional("clustered_index_id", 21, 4),
        required("encrypted", 8, 1),
        required("last_modified_tsn", 23, 8),
        required("current_schema", 21, 4),
    ]
}

fn column_catalog_columns() -> Vec<SyntheticColumn> {
    vec![
        required("table_id", 21, 4),
        required("column_id", 21, 4),
        required("domain_id", 1, 2),
        required("nulls", 8, 1),
        required("width", 21, 4),
        required("scale", 1, 2),
        required("object_id", 23, 8),
        required("max_identity", 20, 8),
        required("column_name", 8, 128),
        optional("default", 10, 32767),
        optional("user_type", 1, 2),
        required("column_type", 8, 1),
        required("compressed", 19, 1),
        required("collect_stats", 19, 1),
        optional("inline_max", 1, 2),
        optional("inline_long", 1, 2),
        optional("lob_index", 19, 1),
        optional("nonmaterialized_value", 12, 32767),
        required("start_schema", 21, 4),
    ]
}

/// The table catalog's row for one table.
fn table_row(entry: &Entry<'_>) -> SyntheticRow {
    let page_count = entry.pages.as_ref().map_or(0, Vec::len) as i32;
    let page_list_cell = match (&entry.pages, entry.table.long_page_list) {
        (Some(pages), Some(storage)) => Cell::long(&page_list(pages), storage),
        (Some(pages), None) => Cell::bytes(&page_list(pages)),
        (None, _) => Cell::Null,
    };
    let extension_pages = &entry.extension_pages;
    let extension_list = if extension_pages.is_empty() {
        Cell::Null
    } else {
        Cell::bytes(&page_list(extension_pages))
    };
    SyntheticRow::new([
        Cell::unsigned_int(entry.id),
        Cell::small_int(0),
        Cell::unsigned_big_int(entry.rows),
        Cell::unsigned_int(entry.table.owner),
        Cell::integer(page_count),
        Cell::integer(extension_pages.len() as i32),
        Cell::integer(0),
        Cell::integer(5),
        Cell::unsigned_big_int(entry.id.into()),
        Cell::timestamp(222_917_760, 0),
        Cell::text(&entry.table.name),
        Cell::tiny_int(entry.table.kind),
        Cell::text("N"),
        Cell::tiny_int(1),
        page_list_cell,
        extension_list,
        Cell::Null,
        Cell::Null,
        Cell::text("N"),
        Cell::unsigned_big_int(0),
        Cell::unsigned_int(entry.table.schema()),
    ])
}

/// The column catalog's row for one column.
fn column_row(table: u32, number: u32, column: &SyntheticColumn) -> SyntheticRow {
    let default = column.default.as_deref().map(Cell::text);
    SyntheticRow::new([
        Cell::unsigned_int(table),
        Cell::unsigned_int(number),
        Cell::small_int(column.domain as i16),
        Cell::text(if column.nullable { "Y" } else { "N" }),
        Cell::unsigned_int(column.width),
        Cell::small_int(column.scale as i16),
        Cell::unsigned_big_int(u64::from(table) << 16 | u64::from(number)),
        Cell::big_int(0),
        Cell::text(&column.name),
        default.unwrap_or(Cell::Null),
        Cell::Null,
        Cell::text("R"),
        Cell::tiny_int(0),
        Cell::tiny_int(1),
        Cell::Null,
        Cell::Null,
        Cell::Null,
        column.absent.clone().unwrap_or(Cell::Null),
        Cell::unsigned_int(column.first_schema),
    ])
}

/// A page list: its size in bits, then control words that each skip some
/// 32-page words and announce how many words of bits follow.
fn page_list(pages: &[u32]) -> Vec<u8> {
    // The most literal words that one control word announces.
    const LITERALS: usize = 63;
    let bits = pages.iter().max().map_or(0, |page| page + 1);
    let bits = bits.next_multiple_of(PAGE_LIST_BITS);
    let mut words = vec![0_u32; bits as usize / 32];
    for page in pages {
        words[*page as usize / 32] |= 1 << (page % 32);
    }
    let mut list = vec![bits];
    let mut rest = &words[..];
    while !rest.is_empty() {
        let (skipped, after) = rest.split_at(run(rest, |word| word == 0));
        let (literals, after) = after.split_at(run(after, |word| word != 0).min(LITERALS));
        list.push((skipped.len() as u32) << 7 | literals.len() as u32);
        list.extend(literals);
        rest = after;
    }
    list.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// How many words `words` starts with that are such ones.
fn run(words: &[u32], such: impl Fn(u32) -> bool) -> usize {
    words.iter().take_while(|&&word| such(word)).count()
}

/// Page 10: a header, then the page list of the table catalog. The header
/// word that counts rows on a row page holds where the list ends.
fn write_page_list(page: &mut [u8; PAGE], pages: &[u32]) {
    const STREAM: usize = 24;
    let list = page_list(pages);
    page[..4].copy_from_slice(&TABLE_PAGE_LIST.to_le_bytes());
    page[6] = b'H';
    page[16] = 20;
    page[18..20].copy_from_slice(&((STREAM + list.len()) as u16).to_le_bytes());
    page[STREAM..STREAM + list.len()].copy_from_slice(&list);
}

/// Page 0: the format numbers, the size of the database, the names of the
/// character sets and the engine's banner. It is stored as it is.
fn write_superblock(page: &mut [u8; PAGE], database_pages: u32) {
    const BANNER: &[u8] = b"2182 SAP SE, Copyright (c)2015 17.0.4.";
    page[0x06] = 0x09;
    page[0x08..0x0c].copy_from_slice(&5000_u32.to_le_bytes());
    page[0x10..0x14].copy_from_slice(&3_u32.to_le_bytes());
    page[0x14..0x18].copy_from_slice(&0xda7a_ba5e_u32.to_le_bytes());
    page[0x18..0x1a].copy_from_slice(&201_u16.to_le_bytes());
    page[0x1a..0x1c].copy_from_slice(&12_u16.to_le_bytes());
    page[0x1c..0x20].copy_from_slice(&database_pages.to_le_bytes());
    page[0x16c..0x176].copy_from_slice(b"1252LATIN1");
    page[0x19c..0x1a8].copy_from_slice(b"windows-1252");
    let banner = BANNER.iter().cycle();
    for (byte, banner) in page[0x400..PAGE - 4].iter_mut().zip(banner) {
        *byte = *banner;
    }
}
