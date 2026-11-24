//! What answers for the tables of a database, a file or something over one.

use std::sync::Arc;
use std::thread;

use sa17::{
    Columns, Database, Error, PageStore, Result, Row, RowAddress, RowId, Rows, Source, Table,
    TableDefinition, TextEncoding, Value,
};

use crate::support;

#[test]
fn columns_belong_to_one_table_of_one_file() {
    let database = support::open(&support::sky());
    let again = support::open(&support::sky());
    let [planets, moons] = ["planets", "moons"].map(|name| support::definition(&database, name));
    let columns = Columns::new(planets, ["id"]).unwrap();
    let missing = Columns::new(planets, ["id", "weight"]).unwrap_err();
    assert_eq!(
        missing.to_string(),
        "the table planets has no column called weight"
    );

    let refused = |mut rows: Box<dyn Iterator<Item = Result<Row>> + Send + '_>| {
        let error = rows.next().unwrap().unwrap_err();
        assert!(rows.next().is_none());
        error
    };
    let source: &dyn Source = &database;
    let of_another = refused(source.scan(moons, &columns));
    assert_eq!(
        of_another.to_string(),
        "the columns are ones of a table planets, and not of this table moons"
    );
    let theirs = support::definition(&again, "planets");
    let their_columns = Columns::all(theirs);
    let of_another_file = refused(source.scan(planets, &their_columns));
    assert!(matches!(of_another_file, Error::WrongColumns { .. }));
    let another_file = refused(source.scan(theirs, &their_columns));
    assert!(matches!(&another_file, Error::WrongDatabase { table } if table == "planets"));
    let address = RowAddress::new(planets.pages()[0], 0);
    for id in [address.into(), RowId::Added(0)] {
        let fetched = source.fetch(planets, &their_columns, id);
        assert!(matches!(fetched, Err(Error::WrongColumns { .. })), "{id}");
        let fetched = source.fetch(theirs, &their_columns, id);
        assert!(matches!(fetched, Err(Error::WrongDatabase { .. })), "{id}");
    }
    // A copy of a definition is still the same table's.
    assert_eq!(source.scan(&planets.clone(), &columns).count(), 3);
}

/// What a source answers for the planets: how many rows a scan finds, and
/// whether each of them is found again on its own.
fn planets<S: Source>(source: S) -> (usize, bool) {
    let table = source.definition("planets").unwrap();
    assert!(source.definitions().contains(table));
    assert_eq!(source.definition_by_id(table.id()), Some(table));
    assert_eq!(source.text_encoding(), TextEncoding::Windows1252);
    let columns = Columns::new(table, ["id"]).unwrap();
    let rows: Vec<Row> = source.scan(table, &columns).map(Result::unwrap).collect();
    let found_again = rows.iter().all(|row| {
        let again = source.fetch(table, &columns, row.id()).unwrap();
        again.as_ref() == Some(row)
    });
    (rows.len(), found_again)
}

#[test]
fn a_pointer_to_a_source_is_a_source() {
    let database = support::open(&support::sky());
    let identity = Some(database.store().identity());
    assert_eq!(planets(&database), (3, true));
    assert_eq!(planets(&database as &dyn Source), (3, true));
    assert_eq!(database.identity(), identity);
    let shared = Arc::new(database);
    assert_eq!(planets(Arc::clone(&shared)), (3, true));
    let boxed: Box<dyn Source> = Box::new(shared);
    assert_eq!(boxed.identity(), identity);
    assert_eq!(planets(&boxed), (3, true));
    assert_eq!(planets(Silent(boxed)), (3, true));
}

/// A source that does not say which file it answers for.
struct Silent(Box<dyn Source>);

impl Source for Silent {
    fn definitions(&self) -> &[TableDefinition] {
        self.0.definitions()
    }

    fn scan<'source>(
        &'source self,
        table: &'source TableDefinition,
        columns: &'source Columns,
    ) -> Box<dyn Iterator<Item = Result<Row>> + Send + 'source> {
        self.0.scan(table, columns)
    }

    fn fetch(&self, table: &TableDefinition, columns: &Columns, id: RowId) -> Result<Option<Row>> {
        self.0.fetch(table, columns, id)
    }

    fn text_encoding(&self) -> TextEncoding {
        self.0.text_encoding()
    }
}

#[test]
fn a_source_need_not_say_which_file_it_answers_for() {
    let silent = Silent(Box::new(support::open(&support::sky())));
    assert_eq!(silent.identity(), None);
    assert_eq!((&silent as &dyn Source).identity(), None);
}

// A program can read one file from many threads.
#[test]
fn a_database_is_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Database>();
    shared::<PageStore>();
    shared::<Table<'_>>();
    shared::<Rows<'_>>();
    shared::<Columns>();
    shared::<Row>();
    shared::<Value>();
    shared::<Error>();
    shared::<Arc<dyn Source>>();

    let database = support::open(&support::sky());
    let planets = database.table("planets").unwrap();
    let every = Columns::all(planets.definition());
    let read = || {
        let addresses = planets.row_addresses().map(Result::unwrap);
        let rows = addresses.map(|address| planets.row(address, &every).unwrap().unwrap());
        rows.collect::<Vec<_>>()
    };
    let here = read();
    thread::scope(|scope| {
        let threads: Vec<_> = (0..4).map(|_| scope.spawn(read)).collect();
        for thread in threads {
            assert_eq!(thread.join().unwrap(), here);
        }
    });
}

// A program that holds any source can read it in another thread than it asks in.
#[test]
fn a_scan_of_any_source_goes_to_another_thread() {
    let source: Arc<dyn Source> = Arc::new(support::open(&support::sky()));
    let planets = source.definition("planets").unwrap();
    let every = Columns::all(planets);
    let rows = source.scan(planets, &every);
    let count = thread::scope(|scope| scope.spawn(|| rows.count()).join().unwrap());
    assert_eq!(count, 3);
    let elsewhere = Arc::clone(&source);
    let count = thread::spawn(move || elsewhere.definitions().len());
    assert_eq!(count.join().unwrap(), source.definitions().len());
}

#[test]
fn owned_cursors_match_borrowed_scans_across_threads() {
    let database = Arc::new(support::open(&support::sky()));
    let planets = database.table("planets").unwrap();
    let definition = planets.definition().clone();
    let columns = Columns::all(&definition);
    let expected_rows: Vec<Row> = planets.scan(&columns).map(Result::unwrap).collect();
    let expected_addresses: Vec<_> = planets.row_addresses().map(Result::unwrap).collect();

    let owned_rows = database.scan_owned(&definition, &columns);
    let owned_addresses = database.row_addresses_owned(&definition);
    let (rows, addresses) = thread::spawn(move || {
        let rows: Vec<Row> = owned_rows.map(Result::unwrap).collect();
        let addresses: Vec<_> = owned_addresses.map(Result::unwrap).collect();
        (rows, addresses)
    })
    .join()
    .unwrap();
    assert_eq!(rows, expected_rows);
    assert_eq!(addresses, expected_addresses);

    // A definition from another open database yields WrongDatabase cleanly.
    let other = Arc::new(support::open(&support::sky()));
    let refused = other.scan_owned(&definition, &columns).next().unwrap();
    assert!(matches!(refused, Err(Error::WrongDatabase { .. })));
}
