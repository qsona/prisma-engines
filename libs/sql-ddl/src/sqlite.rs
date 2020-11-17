use crate::common::{IteratorJoin, SQL_INDENTATION};
use std::{borrow::Cow, fmt::Display};

struct SqliteIdentifier<T>(T);

impl<T: Display> Display for SqliteIdentifier<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

#[derive(Debug, Default)]
pub struct CreateTable<'a> {
    pub table_name: Cow<'a, str>,
    pub columns: Vec<Column<'a>>,
    pub primary_key: Option<Vec<Cow<'a, str>>>,
    pub foreign_keys: Vec<ForeignKey<'a>>,
}

impl Display for CreateTable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CREATE TABLE \"{}\" (\n", self.table_name)?;

        let mut columns = self.columns.iter().peekable();

        while let Some(column) = columns.next() {
            write!(
                f,
                "{indentation}{column}",
                indentation = SQL_INDENTATION,
                column = column
            )?;

            if columns.peek().is_some() {
                f.write_str(",\n")?;
            }
        }

        if let Some(primary_key) = &self.primary_key {
            write!(
                f,
                ",\n\n{indentation}PRIMARY KEY ({columns})",
                indentation = SQL_INDENTATION,
                columns = primary_key.iter().map(SqliteIdentifier).join(", ")
            )?;
        }

        for foreign_key in &self.foreign_keys {
            write!(
                f,
                ",\n{indentation}{fk}",
                indentation = SQL_INDENTATION,
                fk = foreign_key
            )?;
        }

        write!(f, "\n)")
    }
}

#[derive(Debug, Default)]
pub struct ForeignKey<'a> {
    pub constrains: Vec<Cow<'a, str>>,
    pub references: (Cow<'a, str>, Vec<Cow<'a, str>>),
    pub constraint_name: Option<Cow<'a, str>>,
    pub on_delete: Option<ForeignKeyAction>,
}

/// Foreign key action types (for ON DELETE|ON UPDATE).
#[derive(Debug)]
pub enum ForeignKeyAction {
    /// Produce an error indicating that the deletion or update would create a foreign key
    /// constraint violation. If the constraint is deferred, this error will be produced at
    /// constraint check time if there still exist any referencing rows. This is the default action.
    NoAction,
    /// Produce an error indicating that the deletion or update would create a foreign key
    /// constraint violation. This is the same as NO ACTION except that the check is not deferrable.
    Restrict,
    /// Delete any rows referencing the deleted row, or update the values of the referencing
    /// column(s) to the new values of the referenced columns, respectively.
    Cascade,
    /// Set the referencing column(s) to null.
    SetNull,
    /// Set the referencing column(s) to their default values. (There must be a row in the
    /// referenced table matching the default values, if they are not null, or the operation
    /// will fail).
    SetDefault,
}

impl Display for ForeignKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(constraint_name) = &self.constraint_name {
            write!(f, "CONSTRAINT \"{}\" ", constraint_name)?;
        }

        f.write_str("FOREIGN KEY (")?;

        let mut constrained_columns = self.constrains.iter().peekable();

        while let Some(constrained_column) = constrained_columns.next() {
            write!(f, "{}", SqliteIdentifier(constrained_column))?;

            if constrained_columns.peek().is_some() {
                f.write_str(", ")?;
            }
        }

        write!(
            f,
            ") REFERENCES \"{referenced_table}\" (",
            referenced_table = self.references.0,
        )?;

        let mut referenced_columns = self.references.1.iter().peekable();

        while let Some(referenced_column) = referenced_columns.next() {
            write!(f, "{}", SqliteIdentifier(referenced_column))?;

            if referenced_columns.peek().is_some() {
                f.write_str(", ")?;
            }
        }

        f.write_str(")")?;

        if let Some(action) = &self.on_delete {
            match action {
                ForeignKeyAction::NoAction => (),
                ForeignKeyAction::Restrict => f.write_str(" ON DELETE RESTRICT")?,
                ForeignKeyAction::Cascade => f.write_str(" ON DELETE CASCADE")?,
                ForeignKeyAction::SetNull => f.write_str(" ON DELETE SET NULL")?,
                ForeignKeyAction::SetDefault => f.write_str(" ON DELETE SET DEFAULT")?,
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Column<'a> {
    pub name: Cow<'a, str>,
    pub r#type: Cow<'a, str>,
    pub not_null: bool,
    pub primary_key: bool,
    pub default: Option<Cow<'a, str>>,
}

impl Display for Column<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\"{name}\" {tpe}{not_null}{primary_key}",
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
        let create_table = CreateTable {
            table_name: "Cat".into(),
            columns: vec![
                Column {
                    name: "id".into(),
                    r#type: "integer".into(),
                    primary_key: true,
                    ..Default::default()
                },
                Column {
                    name: "boxId".into(),
                    r#type: "uuid".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

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
        let create_table = CreateTable {
            table_name: "Cat".into(),
            columns: vec![
                Column {
                    name: "id".into(),
                    r#type: "integer".into(),
                    ..Default::default()
                },
                Column {
                    name: "boxId".into(),
                    r#type: "uuid".into(),
                    default: Some("'maybe_a_uuid_idk'".into()),
                    ..Default::default()
                },
            ],
            primary_key: Some(vec!["id".into(), "boxId".into()]),
            ..Default::default()
        };

        let expected = r#"
CREATE TABLE "Cat" (
    "id" integer,
    "boxId" uuid DEFAULT 'maybe_a_uuid_idk',

    PRIMARY KEY ("id", "boxId")
)
"#;

        assert_eq!(create_table.to_string(), expected.trim_matches('\n'))
    }

    #[test]
    fn create_table_with_primary_key_and_foreign_keys() {
        let create_table = CreateTable {
            table_name: "Cat".into(),
            columns: vec![
                Column {
                    name: "id".into(),
                    r#type: "integer".into(),
                    ..Default::default()
                },
                Column {
                    name: "boxId".into(),
                    r#type: "uuid".into(),
                    default: Some("'maybe_a_uuid_idk'".into()),
                    ..Default::default()
                },
            ],
            primary_key: Some(vec!["id".into(), "boxId".into()]),
            foreign_keys: vec![
                ForeignKey {
                    constrains: vec!["boxId".into()],
                    references: ("Box".into(), vec!["id".into(), "material".into()]),
                    ..Default::default()
                },
                ForeignKey {
                    constrains: vec!["id".into()],
                    references: ("meow".into(), vec!["id".into()]),
                    constraint_name: Some("meowConstraint".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let expected = r#"
CREATE TABLE "Cat" (
    "id" integer,
    "boxId" uuid DEFAULT 'maybe_a_uuid_idk',

    PRIMARY KEY ("id", "boxId"),
    FOREIGN KEY ("boxId") REFERENCES "Box" ("id", "material"),
    CONSTRAINT "meowConstraint" FOREIGN KEY ("id") REFERENCES "meow" ("id")
)
"#;

        assert_eq!(create_table.to_string(), expected.trim_matches('\n'))
    }
}
