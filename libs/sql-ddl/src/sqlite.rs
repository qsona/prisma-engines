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
        f.write_str(&self.columns)?;
        write!(f, "\n)")
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

    pub fn with_columns<I: IntoIterator<Item = Column<T>>>(mut self, columns: I) -> Self {
        self.columns = columns.into_iter().join(",\n");

        self
    }
}

pub struct Column<T> {
    name: T,
    r#type: T,
    not_null: bool,
    primary_key: bool,
}

impl<T> Column<T> {
    pub fn new(name: T, r#type: T) -> Self {
        Column {
            name,
            r#type,
            not_null: false,
            primary_key: false,
        }
    }

    pub fn primary_key(mut self, is_pk: bool) -> Self {
        self.primary_key = is_pk;

        self
    }
}

impl<T: Display> Display for Column<T> {
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
        let create_table = CreateTable::named("Cat").with_columns(
            std::iter::once(Column::new("id", "integer").primary_key(true))
                .chain(std::iter::once(Column::new("boxId", "uuid"))),
        );

        let expected = r#"
CREATE TABLE "Cat" (
    id integer PRIMARY KEY,
    boxId uuid
)
"#;

        assert_eq!(create_table.to_string(), expected.trim_matches('\n'))
    }
}
