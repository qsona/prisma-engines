use crate::common::{IteratorJoin, SQL_INDENTATION};
use std::fmt::{Display, Write as _};

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

    pub fn with_columns<I: IntoIterator<Item = SqliteColumn<T>>>(mut self, columns: I) -> Self {
        self.columns = columns.into_iter().join(",\n");

        self
    }
}

pub struct SqliteColumn<T> {
    name: T,
    r#type: T,
    not_null: bool,
    primary_key: bool,
}

impl<T: Display> Display for SqliteColumn<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{indentation}{name} {tpe}{not_null}{primary_key}",
            indentation = SQL_INDENTATION,
            name = self.name,
            tpe = self.r#type,
            not_null = if self.not_null { " NOT NULL" } else { "" },
            primary_key = if self.primary_key { " PRIMARY KEY" } else { "" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_create_table() {
        todo!()
    }
}
