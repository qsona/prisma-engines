use std::fmt::Display;

pub struct CreateTable<T> {
    table_name: T,
    columns: String,
    primary_key: Option<String>,
}

impl<T> Display for CreateTable<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CREATE TABLE \"{}\" (\n", self.table_name)?;
        write!(f, ")")
    }
}

impl<T: Display> CreateTable<T> {
    pub fn named(table_name: T) -> Self {
        CreateTable {
            table_name,
            columns: String::new(),
            primary_key: None,
        }
    }

    pub fn with_columns<F: FnOnce(SqliteColumns) -> SqliteColumns>(mut self, columns: F) -> Self {
        self.columns = columns(SqliteColumns(String::new())).0;

        self
    }
}

pub struct SqliteColumns(String);

impl SqliteColumns {
    pub fn column<T>(self, name: T, r#type: T, column_options: SqliteColumnOptions) -> Self {
        self
    }
}

#[derive(Debug, Default)]
pub struct SqliteColumnOptions {
    not_null: bool,
    on_delete: Option<()>,
    check: Option<()>,
}
