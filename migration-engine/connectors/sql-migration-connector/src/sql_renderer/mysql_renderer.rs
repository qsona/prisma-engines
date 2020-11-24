use super::{
    common::SQL_INDENTATION,
    common::{render_nullability, render_on_delete, Quoted},
    IteratorJoin, SqlRenderer,
};
use crate::{
    flavour::{MysqlFlavour, SqlFlavour, MYSQL_IDENTIFIER_SIZE_LIMIT},
    pair::Pair,
    sql_migration::{
        expanded_alter_column::{expand_mysql_alter_column, MysqlAlterColumn},
        AddColumn, AlterColumn, AlterEnum, AlterTable, DropColumn, RedefineTable, TableChange,
    },
    sql_schema_differ::ColumnChanges,
};
use once_cell::sync::Lazy;
use prisma_value::PrismaValue;
use regex::Regex;
use sql_schema_describer::{
    walkers::{ColumnWalker, EnumWalker, ForeignKeyWalker, IndexWalker, TableWalker},
    ColumnTypeFamily, DefaultKind, DefaultValue, IndexType, SqlSchema,
};
use std::borrow::Cow;

const VARCHAR_LENGTH_PREFIX: &str = "(191)";

impl SqlRenderer for MysqlFlavour {
    fn quote<'a>(&self, name: &'a str) -> Quoted<&'a str> {
        Quoted::Backticks(name)
    }

    fn render_add_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        let constraint_clause = foreign_key
            .constraint_name()
            .map(|constraint_name| format!("CONSTRAINT {} ", self.quote(constraint_name)))
            .unwrap_or_else(String::new);

        let columns = foreign_key
            .constrained_column_names()
            .iter()
            .map(|col| self.quote(col))
            .join(", ");

        format!(
            "ALTER TABLE `{table}` ADD {constraint_clause}FOREIGN KEY ({columns}){references}",
            table = foreign_key.table().name(),
            constraint_clause = constraint_clause,
            columns = columns,
            references = self.render_references(foreign_key),
        )
    }

    fn render_alter_enum(&self, _alter_enum: &AlterEnum, _differ: &Pair<&SqlSchema>) -> Vec<String> {
        unreachable!("render_alter_enum on MySQL")
    }

    fn render_alter_index(&self, indexes: Pair<&IndexWalker<'_>>) -> Vec<String> {
        vec![format!(
            "ALTER TABLE {table_name} RENAME INDEX {index_name} TO {index_new_name}",
            table_name = self.quote(indexes.previous().table().name()),
            index_name = self.quote(indexes.previous().name()),
            index_new_name = self.quote(indexes.next().name())
        )]
    }

    fn render_alter_table(&self, alter_table: &AlterTable, schemas: &Pair<&SqlSchema>) -> Vec<String> {
        let AlterTable { table_index, changes } = alter_table;

        let tables = schemas.tables(table_index);

        let mut lines = Vec::new();

        for change in changes {
            match change {
                TableChange::DropPrimaryKey => lines.push("DROP PRIMARY KEY".to_owned()),
                TableChange::AddPrimaryKey { columns } => lines.push(format!(
                    "ADD PRIMARY KEY ({})",
                    columns.iter().map(|colname| self.quote(colname)).join(", ")
                )),
                TableChange::AddColumn(AddColumn { column_index }) => {
                    let column = tables.next().column_at(*column_index);
                    let col_sql = self.render_column(&column);

                    lines.push(format!("ADD COLUMN {}", col_sql));
                }
                TableChange::DropColumn(DropColumn { index }) => {
                    let name = self.quote(tables.previous().column_at(*index).name());
                    lines.push(format!("DROP COLUMN {}", name));
                }
                TableChange::AlterColumn(AlterColumn {
                    changes,
                    column_index,
                    type_change: _,
                }) => {
                    let columns = tables.columns(column_index);
                    let expanded = expand_mysql_alter_column(&columns, &changes);

                    match expanded {
                        MysqlAlterColumn::DropDefault => lines.push(format!(
                            "ALTER COLUMN {column} DROP DEFAULT",
                            column = Quoted::mysql_ident(columns.previous().name())
                        )),
                        MysqlAlterColumn::Modify { new_default, changes } => lines.push(render_mysql_modify(
                            &changes,
                            new_default.as_ref(),
                            columns.next(),
                            self,
                        )),
                    };
                }
                TableChange::DropAndRecreateColumn { .. } => unreachable!("DropAndRecreateColumn on MySQL"),
            };
        }

        if lines.is_empty() {
            return Vec::new();
        }

        vec![format!(
            "ALTER TABLE {} {}",
            self.quote(tables.previous().name()),
            lines.join(",\n    ")
        )]
    }

    fn render_column(&self, column: &ColumnWalker<'_>) -> String {
        let column_name = self.quote(column.name());
        let tpe_str = render_column_type(&column);
        let nullability_str = render_nullability(&column);
        let default_str = column
            .default()
            .filter(|default| {
                !matches!(default.kind(), DefaultKind::DBGENERATED(_) | DefaultKind::SEQUENCE(_))
                    // We do not want to render JSON defaults because they are not supported by MySQL.
                    && !matches!(column.column_type_family(), ColumnTypeFamily::Json)
                    // We do not want to render binary defaults because they are not supported by MySQL.
                    && !matches!(column.column_type_family(), ColumnTypeFamily::Binary)
            })
            .map(|default| {
                format!(
                    " DEFAULT {}",
                    self.render_default(default, &column.column_type_family())
                )
            })
            .unwrap_or_else(String::new);
        let foreign_key = column.table().foreign_key_for_column(column.name());
        let auto_increment_str = if column.is_autoincrement() {
            " AUTO_INCREMENT"
        } else {
            ""
        };

        match foreign_key {
            Some(_) => format!(
                "{}{} {}{}{}",
                SQL_INDENTATION, column_name, tpe_str, nullability_str, default_str
            ),
            None => format!(
                "{}{} {}{}{}{}",
                SQL_INDENTATION, column_name, tpe_str, nullability_str, default_str, auto_increment_str
            ),
        }
    }

    fn render_references(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        let referenced_columns = foreign_key
            .referenced_column_names()
            .iter()
            .map(|col| self.quote(col))
            .join(",");

        format!(
            " REFERENCES `{table_name}`({column_names}) {on_delete} ON UPDATE CASCADE",
            table_name = foreign_key.referenced_table().name(),
            column_names = referenced_columns,
            on_delete = render_on_delete(foreign_key.on_delete_action())
        )
    }

    fn render_default<'a>(&self, default: &'a DefaultValue, family: &ColumnTypeFamily) -> Cow<'a, str> {
        match (default.kind(), family) {
            (DefaultKind::DBGENERATED(val), _) => val.as_str().into(),
            (DefaultKind::VALUE(PrismaValue::String(val)), ColumnTypeFamily::String)
            | (DefaultKind::VALUE(PrismaValue::Enum(val)), ColumnTypeFamily::Enum(_)) => {
                format!("'{}'", escape_string_literal(&val)).into()
            }
            (DefaultKind::NOW, ColumnTypeFamily::DateTime) => "CURRENT_TIMESTAMP(3)".into(),
            (DefaultKind::NOW, _) => unreachable!("NOW default on non-datetime column"),
            (DefaultKind::VALUE(val), ColumnTypeFamily::DateTime) => format!("'{}'", val).into(),
            (DefaultKind::VALUE(val), _) => format!("{}", val).into(),
            (DefaultKind::SEQUENCE(_), _) => "".into(),
        }
    }

    fn render_create_enum(&self, _create_enum: &EnumWalker<'_>) -> Vec<String> {
        Vec::new() // enums are defined on each column that uses them on MySQL
    }

    fn render_create_index(&self, index: &IndexWalker<'_>) -> String {
        let name = index.name();
        let name = if name.len() > MYSQL_IDENTIFIER_SIZE_LIMIT {
            &name[0..MYSQL_IDENTIFIER_SIZE_LIMIT]
        } else {
            &name
        };
        let index_type = match index.index_type() {
            IndexType::Unique => "UNIQUE ",
            IndexType::Normal => "",
        };
        let index_name = self.quote(&name);
        let table_reference = self.quote(&index.table().name());

        let columns = index.columns().map(|c| self.quote(c.name()));

        format!(
            "CREATE {index_type}INDEX {index_name} ON {table_reference}({columns})",
            index_type = index_type,
            index_name = index_name,
            table_reference = table_reference,
            columns = columns.join(", ")
        )
    }

    fn render_create_table_as(&self, table: &TableWalker<'_>, table_name: &str) -> String {
        let columns: String = table.columns().map(|column| self.render_column(&column)).join(",\n");

        let primary_columns = table.primary_key_column_names();

        let primary_key = if let Some(primary_columns) = primary_columns.as_ref().filter(|cols| !cols.is_empty()) {
            let column_names = primary_columns.iter().map(|col| self.quote(&col)).join(",");
            format!(",\n\n{}PRIMARY KEY ({})", SQL_INDENTATION, column_names)
        } else {
            String::new()
        };

        let indexes = if !table.indexes().next().is_none() {
            let indices: String = table
                .indexes()
                .map(|index| {
                    let tpe = if index.index_type().is_unique() { "UNIQUE " } else { "" };
                    let index_name = if index.name().len() > MYSQL_IDENTIFIER_SIZE_LIMIT {
                        &index.name()[0..MYSQL_IDENTIFIER_SIZE_LIMIT]
                    } else {
                        &index.name()
                    };

                    format!(
                        "{}INDEX {}({})",
                        tpe,
                        self.quote(&index_name),
                        index.columns().map(|col| self.quote(col.name())).join(",\n")
                    )
                })
                .join(",\n");

            format!(",\n{}", indices)
        } else {
            String::new()
        };

        format!(
            "CREATE TABLE {} (\n{columns}{indexes}{primary_key}\n) DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            table_name = self.quote(table_name),
            columns = columns,
            indexes = indexes,
            primary_key = primary_key,
        )
    }

    fn render_drop_and_recreate_index(&self, indexes: Pair<&IndexWalker<'_>>) -> Vec<String> {
        // Order matters: dropping the old index first wouldn't work when foreign key constraints are still relying on it.
        vec![
            self.render_create_index(indexes.next()),
            mysql_drop_index(indexes.previous().table().name(), indexes.previous().name()),
        ]
    }

    fn render_drop_enum(&self, _: &EnumWalker<'_>) -> Vec<String> {
        Vec::new()
    }

    fn render_drop_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        format!(
            "ALTER TABLE {table} DROP FOREIGN KEY {constraint_name}",
            table = self.quote(foreign_key.table().name()),
            constraint_name = Quoted::mysql_ident(foreign_key.constraint_name().unwrap()),
        )
    }

    fn render_drop_index(&self, index: &IndexWalker<'_>) -> String {
        mysql_drop_index(&index.table().name(), &index.name())
    }

    fn render_drop_table(&self, table_name: &str) -> Vec<String> {
        vec![format!("DROP TABLE {}", self.quote(&table_name))]
    }

    fn render_redefine_tables(&self, _names: &[RedefineTable], _schemas: &Pair<&SqlSchema>) -> Vec<String> {
        unreachable!("render_redefine_table on MySQL")
    }

    fn render_rename_table(&self, name: &str, new_name: &str) -> String {
        format!(
            "ALTER TABLE {} RENAME TO {}",
            self.quote(name),
            new_name = self.quote(new_name),
        )
    }
}

fn render_mysql_modify(
    changes: &ColumnChanges,
    new_default: Option<&sql_schema_describer::DefaultValue>,
    next_column: &ColumnWalker<'_>,
    renderer: &dyn SqlFlavour,
) -> String {
    let column_type: Option<String> = if changes.type_changed() {
        Some(next_column.column_type().full_data_type.clone()).filter(|r| !r.is_empty() || r.contains("datetime"))
    // @default(now()) does not work with datetimes of certain sizes
    } else {
        Some(next_column.column_type().full_data_type.clone()).filter(|r| !r.is_empty())
    };

    let column_type = column_type
        .map(Cow::Owned)
        .unwrap_or_else(|| render_column_type(&next_column));

    let default = new_default
        .map(|default| renderer.render_default(&default, &next_column.column_type().family))
        .filter(|expr| !expr.is_empty())
        .map(|expression| format!(" DEFAULT {}", expression))
        .unwrap_or_else(String::new);

    format!(
        "MODIFY {column_name} {column_type}{nullability}{default}{sequence}",
        column_name = Quoted::mysql_ident(&next_column.name()),
        column_type = column_type,
        nullability = if next_column.arity().is_required() {
            " NOT NULL"
        } else {
            ""
        },
        default = default,
        sequence = if next_column.is_autoincrement() {
            " AUTO_INCREMENT"
        } else {
            ""
        },
    )
}

pub(crate) fn render_column_type(column: &ColumnWalker<'_>) -> Cow<'static, str> {
    if !column.column_type().full_data_type.is_empty() {
        return column.column_type().full_data_type.clone().into();
    }

    match &column.column_type().family {
        ColumnTypeFamily::Boolean => "BOOLEAN".into(),
        ColumnTypeFamily::DateTime => "DATETIME(3)".into(),
        ColumnTypeFamily::Float => "DECIMAL(65,30)".into(),
        ColumnTypeFamily::Decimal => "DECIMAL(65,30)".into(),
        ColumnTypeFamily::Int => "INT".into(),
        ColumnTypeFamily::BigInt => "BIGINT".into(),
        // we use varchar right now as mediumtext doesn't allow default values
        // a bigger length would not allow to use such a column as primary key
        ColumnTypeFamily::String => format!("VARCHAR{}", VARCHAR_LENGTH_PREFIX).into(),
        ColumnTypeFamily::Enum(enum_name) => {
            let r#enum = column
                .schema()
                .get_enum(&enum_name)
                .unwrap_or_else(|| panic!("Could not render the variants of enum `{}`", enum_name));

            let variants: String = r#enum.values.iter().map(Quoted::mysql_string).join(", ");

            format!("ENUM({})", variants).into()
        }
        ColumnTypeFamily::Json => "JSON".into(),
        ColumnTypeFamily::Binary => "LONGBLOB".into(),
        ColumnTypeFamily::Uuid => unimplemented!("Uuid not handled yet"),
        ColumnTypeFamily::Unsupported(x) => unimplemented!("{} not handled yet", x),
    }
}

fn escape_string_literal(s: &str) -> Cow<'_, str> {
    static STRING_LITERAL_CHARACTER_TO_ESCAPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"'"#).unwrap());

    STRING_LITERAL_CHARACTER_TO_ESCAPE_RE.replace_all(s, "'$0")
}

fn mysql_drop_index(table_name: &str, index_name: &str) -> String {
    format!("DROP INDEX `{}` ON `{}`", index_name, table_name)
}
