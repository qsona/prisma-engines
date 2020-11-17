use crate::common::{IteratorJoin, SQL_INDENTATION};
use std::{borrow::Cow, fmt::Display};

struct SqliteIdentifier<T>(T);

impl<T: Display> Display for SqliteIdentifier<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

pub struct CreateTable<'a> {
    table_name: Cow<'a, str>,
    columns: Vec<Column<'a>>,
    primary_key: Option<String>,
}

impl<T> Display for CreateTable<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CREATE TABLE \"{}\" (\n", self.table_name)?;
        f.write_str(&self.columns)?;

        if let Some(primary_key) = &self.primary_key {
            write!(
                f,
                ",\n\n{indentation}PRIMARY KEY ({columns})",
                indentation = SQL_INDENTATION,
                columns = primary_key
            )?;
        }

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

    pub fn columns<I, U, V>(mut self, columns: I) -> Self
    where
        U: Display,
        V: Display,
        I: Iterator<Item = Column<U, V>>,
    {
        self.columns = columns.into_iter().join(",\n");

        self
    }

    pub fn primary_key<U: Display>(mut self, columns: impl Iterator<Item = U>) -> Self {
        self.primary_key = Some(columns.into_iter().map(SqliteIdentifier).join(", "));

        self
    }
}

pub struct Column<'a> {
    pub name: Cow<'a, str>,
    pub r#type: Cow<'a, str>,
    pub not_null: bool,
    pub primary_key: bool,
    pub default: Option<U>,
}

impl<'a> Column<'a> {
    pub fn new(name: Cow<'a, str>, r#type: Cow<'a, str>) -> Self {
        Column {
            name,
            r#type,
            not_null: false,
            primary_key: false,
            default: None,
        }
    }

    pub fn default(mut self, default: Option<U>) -> Self {
        self.default = default;

        self
    }

    pub fn not_null(mut self, not_null: bool) -> Self {
        self.not_null = not_null;

        self
    }

    pub fn primary_key(mut self, is_pk: bool) -> Self {
        self.primary_key = is_pk;

        self
    }
}

impl<T: Display, U: Display> Display for Column<T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{indentation}\"{name}\" {tpe}{not_null}{primary_key}",
            indentation = SQL_INDENTATION,
            name = self.name,
            tpe = self.r#type,
            not_null = if self.not_null { " NOT NULL" } else { "" },
            primary_key = if self.primary_key { " PRIMARY KEY" } else { "" },
        )?;

        if let Some(default) = &self.default {
            write!(f, " DEFAULT {}", default)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_create_table() {
        let create_table = CreateTable::named("Cat").columns(
            std::iter::once(Column::new("id", "integer").primary_key(true))
                .chain(std::iter::once(Column::new("boxId", "uuid"))),
        );

        let expected = r#"
CREATE TABLE "Cat" (
    "id" integer PRIMARY KEY,
    "boxId" uuid
)
"#;

        assert_eq!(create_table.to_string(), expected.trim_matches('\n'))
    }

    #[test]
    fn create_table_with_primary_key() {
        let create_table = CreateTable::named("Cat")
            .columns(
                std::iter::once(Column::new("id", "integer").primary_key(false)).chain(std::iter::once(
                    Column::new("boxId", "uuid").default(Some("'maybe_a_uuid_idk'")),
                )),
            )
            .primary_key(["id", "boxId"].iter());

        let expected = r#"
CREATE TABLE "Cat" (
    "id" integer,
    "boxId" uuid DEFAULT 'maybe_a_uuid_idk',

    PRIMARY KEY ("id", "boxId")
)
"#;

        assert_eq!(create_table.to_string(), expected.trim_matches('\n'))
    }
}
