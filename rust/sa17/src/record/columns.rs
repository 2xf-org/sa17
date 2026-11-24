//! The columns of a table that a scan reads.

use std::fmt;
use std::iter;
use std::sync::{Arc, OnceLock};

use crate::error::{Error, RowProblem};
use crate::record::Plan;
use crate::schema::{Column, TableDefinition};

/// Columns of one table, chosen once by their names or their positions.
///
/// A scan decodes these and steps over the others, so it pays to ask for few.
/// A row holds its values in the order they were asked for here, whatever
/// their order in the table. Cloning is cheap.
///
/// ```no_run
/// use sa17::{Columns, Database};
///
/// let database = Database::open("garden.db")?;
/// let plants = database.table("plants")?;
/// let columns = Columns::new(plants.definition(), ["name", "height"])?;
/// for row in plants.scan(&columns) {
///     let row = row?;
///     let (name, height): (&str, Option<f64>) = (row.read(0)?, row.read(1)?);
///     println!("{name}: {height:?}");
/// }
/// # Ok::<(), sa17::Error>(())
/// ```
#[derive(Clone)]
pub struct Columns {
    // Every row read through these columns shares them, and outlives the scan.
    shared: Arc<Shared>,
}

struct Shared {
    table_name: String,
    /// The columns of the table, which its definition shares.
    definition: Arc<[Column]>,
    asked: Vec<Asked>,
    /// For each column of the table, the place of the first time it was asked for.
    slots: Vec<Option<usize>>,
    /// The places of columns that were asked for again.
    repeated: Vec<Repeat>,
    /// The schema versions that added columns, in order.
    versions: Vec<u32>,
    /// How to read the rows before each of the versions, and after the last.
    plans: Vec<OnceLock<Result<Plan, RowProblem>>>,
}

/// A column that was asked for more than once: the place that has its value,
/// and one that wants it too.
#[derive(Clone, Copy)]
pub(crate) struct Repeat {
    pub(crate) from: usize,
    pub(crate) to: usize,
}

/// One column that was asked for.
#[derive(PartialEq)]
struct Asked {
    name: String,
    /// Its position in the table, unless the table has no such column.
    position: Option<usize>,
}

impl Columns {
    /// Every column of `table`, in the table's order.
    #[must_use]
    pub fn all(table: &TableDefinition) -> Self {
        let asked = table.columns.iter().enumerate();
        let asked = asked.map(|(position, column)| Asked {
            name: column.name.clone(),
            position: Some(position),
        });
        Self::from_asked(table, asked.collect())
    }

    /// No column of `table`, for what only wants to know its rows.
    #[must_use]
    pub fn none(table: &TableDefinition) -> Self {
        Self::from_asked(table, Vec::new())
    }

    /// The columns of `table` called `names`, ignoring ASCII case. A name that
    /// two columns have is the first one's.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownColumn`] with the first name that no column
    /// has.
    pub fn new<N: AsRef<str>>(
        table: &TableDefinition,
        names: impl IntoIterator<Item = N>,
    ) -> Result<Self, Error> {
        Self::with_optional(table, names, [""; 0])
    }

    /// The columns of `table` at `positions` in its [`TableDefinition::columns`],
    /// such as those of a key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownPosition`] with the first position that no
    /// column is at.
    pub fn at<'position>(
        table: &TableDefinition,
        positions: impl IntoIterator<Item = &'position usize>,
    ) -> Result<Self, Error> {
        let asked = positions.into_iter().map(|&position| {
            let column = table.columns.get(position);
            let column = column.ok_or_else(|| Error::UnknownPosition {
                table: table.name.clone(),
                position,
            })?;
            Ok(Asked {
                name: column.name.clone(),
                position: Some(position),
            })
        });
        let asked: Result<Vec<Asked>, Error> = asked.collect();
        Ok(Self::from_asked(table, asked?))
    }

    /// The columns of `table` called `names`, then the ones called `optional`,
    /// ignoring ASCII case. A column of `optional` that the table does not
    /// have keeps its place, and is `NULL` in every row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownColumn`] with the first of `names` that no
    /// column has.
    pub fn with_optional<N: AsRef<str>, O: AsRef<str>>(
        table: &TableDefinition,
        names: impl IntoIterator<Item = N>,
        optional: impl IntoIterator<Item = O>,
    ) -> Result<Self, Error> {
        let ask = |name: &str| Asked {
            name: name.to_owned(),
            position: table.position(name),
        };
        let mut asked = Vec::new();
        for name in names {
            let column = ask(name.as_ref());
            if column.position.is_none() {
                return Err(Error::UnknownColumn {
                    table: table.name.clone(),
                    column: column.name,
                });
            }
            asked.push(column);
        }
        asked.extend(optional.into_iter().map(|name| ask(name.as_ref())));
        Ok(Self::from_asked(table, asked))
    }

    fn from_asked(table: &TableDefinition, asked: Vec<Asked>) -> Self {
        let mut versions: Vec<u32> = table
            .columns
            .iter()
            .map(|column| column.first_schema)
            .collect();
        versions.sort_unstable();
        versions.dedup();
        let plans = iter::repeat_with(OnceLock::new).take(versions.len() + 1);

        let mut slots = vec![None; table.columns.len()];
        let mut repeated = Vec::new();
        for (slot, column) in asked.iter().enumerate() {
            let Some(place) = column.position.and_then(|position| slots.get_mut(position)) else {
                continue;
            };
            match *place {
                Some(from) => repeated.push(Repeat { from, to: slot }),
                None => *place = Some(slot),
            }
        }
        Self {
            shared: Arc::new(Shared {
                table_name: table.name.clone(),
                definition: Arc::clone(&table.columns),
                asked,
                slots,
                repeated,
                plans: plans.collect(),
                versions,
            }),
        }
    }

    /// The name of the table.
    #[must_use]
    pub fn table_name(&self) -> &str {
        &self.shared.table_name
    }

    /// The number of columns that were asked for.
    #[must_use]
    pub fn len(&self) -> usize {
        self.shared.asked.len()
    }

    /// Whether no column was asked for.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shared.asked.is_empty()
    }

    /// The names, in the order they were asked for.
    pub fn names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.shared.asked.iter().map(|column| column.name.as_str())
    }

    /// The place of the column called `name` among these, ignoring ASCII case.
    #[must_use]
    pub fn position(&self, name: &str) -> Option<usize> {
        let mut names = self.names();
        names.position(|asked| asked.eq_ignore_ascii_case(name))
    }

    /// The column at `position` among these, if the table has it.
    #[must_use]
    pub fn column(&self, position: usize) -> Option<&Column> {
        let position = self.shared.asked.get(position)?.position?;
        self.shared.definition.get(position)
    }

    /// Checks that these are columns of `table`, and not of another one or of
    /// the same one in another open database.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WrongColumns`] when they are not.
    pub fn check(&self, table: &TableDefinition) -> Result<(), Error> {
        if Arc::ptr_eq(&self.shared.definition, &table.columns) {
            Ok(())
        } else {
            Err(Error::WrongColumns {
                table: table.name.clone(),
                chosen_from: self.shared.table_name.clone(),
            })
        }
    }

    /// How to read rows written at `schema`.
    pub(crate) fn plan(&self, schema: u32) -> Result<&Plan, RowProblem> {
        let shared = &*self.shared;
        let older = |version: &u32| *version <= schema;
        let place = shared.versions.partition_point(older);
        let plan = shared.plans.get(place).ok_or(RowProblem::Columns)?;
        let built = plan.get_or_init(|| Plan::new(&shared.definition, &shared.slots, schema));
        built.as_ref().map_err(|problem| *problem)
    }

    /// The columns of the table.
    pub(crate) fn table_columns(&self) -> &[Column] {
        &self.shared.definition
    }

    /// The columns that were asked for more than once.
    pub(crate) fn repeated(&self) -> &[Repeat] {
        &self.shared.repeated
    }
}

impl PartialEq for Columns {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
            || (Arc::ptr_eq(&self.shared.definition, &other.shared.definition)
                && self.shared.asked == other.shared.asked)
    }
}

impl Eq for Columns {}

impl fmt::Debug for Columns {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.table_name())?;
        formatter.debug_list().entries(self.names()).finish()
    }
}
