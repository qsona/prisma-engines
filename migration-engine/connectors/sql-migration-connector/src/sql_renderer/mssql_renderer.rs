use super::{common, IteratorJoin, Quoted, QuotedWithSchema, SqlRenderer};
use crate::{
    flavour::MssqlFlavour,
    pair::Pair,
    sql_migration::{
        expanded_alter_column::{expand_mssql_alter_column, MsSqlAlterColumn},
        AddColumn, AlterColumn, AlterEnum, AlterTable, DropColumn, RedefineTable, TableChange,
    },
};
use indoc::formatdoc;
use prisma_value::PrismaValue;
use sql_schema_describer::{
    walkers::{ColumnWalker, EnumWalker, ForeignKeyWalker, IndexWalker, TableWalker},
    ColumnTypeFamily, DefaultKind, DefaultValue, IndexType, SqlSchema,
};
use std::{borrow::Cow, fmt::Write};

impl MssqlFlavour {
    fn quote_with_schema<'a, 'b>(&'a self, name: &'b str) -> QuotedWithSchema<'a, &'b str> {
        QuotedWithSchema {
            schema_name: self.schema_name(),
            name: self.quote(name),
        }
    }
}

impl SqlRenderer for MssqlFlavour {
    fn quote<'a>(&self, name: &'a str) -> Quoted<&'a str> {
        Quoted::mssql_ident(name)
    }

    fn render_alter_table(&self, alter_table: &AlterTable, schemas: &Pair<&SqlSchema>) -> Vec<String> {
        let AlterTable { table_index, changes } = alter_table;

        let tables = schemas.tables(table_index);

        let mut drop_constraints = Vec::new();
        let mut add_constraints = Vec::new();
        let mut add_columns = Vec::new();
        let mut drop_columns = Vec::new();
        let mut column_mods = Vec::new();

        for change in changes {
            match change {
                TableChange::DropPrimaryKey => {
                    let constraint = tables
                        .previous()
                        .primary_key()
                        .and_then(|pk| pk.constraint_name.as_ref())
                        .expect("Missing constraint name in DropPrimaryKey on MSSQL");

                    drop_constraints.push(format!("{}", self.quote(constraint)));
                }
                TableChange::AddPrimaryKey { columns } => {
                    let non_quoted_columns = columns.iter().map(|colname| colname);

                    let quoted_columns = columns
                        .iter()
                        .map(|colname| self.quote(colname))
                        .map(|column| format!("{}", column));

                    add_constraints.push(format!(
                        "CONSTRAINT PK__{}__{} PRIMARY KEY ({})",
                        tables.next().name(),
                        non_quoted_columns.join("__"),
                        quoted_columns.join(","),
                    ));
                }
                TableChange::AddColumn(AddColumn { column_index }) => {
                    let column = tables.next().column_at(*column_index);
                    add_columns.push(self.render_column(&column));
                }
                TableChange::DropColumn(DropColumn { index }) => {
                    let name = self.quote(tables.previous().column_at(*index).name());
                    drop_columns.push(format!("{}", name));
                }
                TableChange::DropAndRecreateColumn { column_index, .. } => {
                    let columns = tables.columns(column_index);

                    drop_columns.push(format!("{}", self.quote(columns.previous().name())));
                    add_columns.push(self.render_column(columns.next()));
                }
                TableChange::AlterColumn(AlterColumn {
                    column_index,
                    changes,
                    type_change: _,
                }) => {
                    let columns = tables.columns(column_index);
                    let expanded = expand_mssql_alter_column(&columns, &changes);

                    for alter in expanded.into_iter() {
                        match alter {
                            MsSqlAlterColumn::DropDefault { constraint_name } => {
                                drop_constraints.push(constraint_name);
                            }
                            MsSqlAlterColumn::SetDefault(default) => {
                                let default = self.render_default(&default, &columns.next().column_type().family);

                                add_constraints.push(format!(
                                    "CONSTRAINT DF__{table}__{column} DEFAULT {default} FOR {column}",
                                    table = tables.next().name(),
                                    column = columns.next().name(),
                                    default = default,
                                ))
                            }
                            MsSqlAlterColumn::Modify => {
                                let nullability = if columns.next().arity().is_required() {
                                    "NOT NULL"
                                } else {
                                    "NULL"
                                };

                                column_mods.push(format!(
                                    "ALTER TABLE {table} ALTER COLUMN {column_name} {column_type} {nullability}",
                                    table = self.quote_with_schema(tables.previous().name()),
                                    column_name = self.quote(&columns.next().name()),
                                    column_type = render_column_type(columns.next()),
                                    nullability = nullability,
                                ));
                            }
                        }
                    }
                }
            };
        }

        let mut statements = Vec::new();

        // first drop constraints
        if !drop_constraints.is_empty() {
            statements.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT {}",
                self.quote_with_schema(tables.previous().name()),
                drop_constraints.join(",\n"),
            ));
        }

        // then alter cols
        if !column_mods.is_empty() {
            statements.extend(column_mods)
        }

        // finally add new constraints back
        if !add_constraints.is_empty() {
            statements.push(format!(
                "ALTER TABLE {} ADD {}",
                self.quote_with_schema(tables.previous().name()),
                add_constraints.join(", ")
            ));
        }

        if !drop_columns.is_empty() {
            statements.push(format!(
                "ALTER TABLE {} DROP COLUMN {}",
                self.quote_with_schema(tables.previous().name()),
                drop_columns.join(",\n"),
            ));
        }

        if !add_columns.is_empty() {
            statements.push(format!(
                "ALTER TABLE {} ADD {}",
                self.quote_with_schema(tables.previous().name()),
                add_columns.join(",\n"),
            ));
        }

        statements
    }

    fn render_alter_enum(&self, _: &AlterEnum, _: &Pair<&SqlSchema>) -> Vec<String> {
        unreachable!("render_alter_enum on Microsoft SQL Server")
    }

    fn render_column(&self, column: &ColumnWalker<'_>) -> String {
        let column_name = self.quote(column.name());
        let r#type = render_column_type(column);
        let nullability = common::render_nullability(&column);

        let default = column
            .default()
            .filter(|default| !matches!(default.kind(), DefaultKind::DBGENERATED(_)))
            .map(|default| {
                format!(
                    " DEFAULT {}",
                    self.render_default(default, &column.column_type_family())
                )
            })
            .unwrap_or_else(String::new);

        if column.is_autoincrement() {
            format!("{} int IDENTITY(1,1)", column_name)
        } else {
            format!("{} {}{}{}", column_name, r#type, nullability, default)
        }
    }

    fn render_references(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        let cols = foreign_key
            .referenced_column_names()
            .iter()
            .map(Quoted::mssql_ident)
            .join(",");

        let (on_delete, on_update) = if foreign_key.is_a_relation_loop() {
            ("ON DELETE NO ACTION", "ON UPDATE NO ACTION")
        } else {
            let on_delete = common::render_on_delete(&foreign_key.on_delete_action());
            let on_update = common::render_on_update(&foreign_key.on_update_action());

            (on_delete, on_update)
        };

        format!(
            " REFERENCES {}({}) {} {}",
            self.quote_with_schema(&foreign_key.referenced_table().name()),
            cols,
            on_delete,
            on_update
        )
    }

    fn render_default<'a>(&self, default: &'a DefaultValue, family: &ColumnTypeFamily) -> Cow<'a, str> {
        match (default.kind(), family) {
            (DefaultKind::DBGENERATED(val), _) => val.as_str().into(),
            (DefaultKind::VALUE(PrismaValue::String(val)), ColumnTypeFamily::String)
            | (DefaultKind::VALUE(PrismaValue::Enum(val)), ColumnTypeFamily::Enum(_)) => {
                format!("'{}'", escape_string_literal(&val)).into()
            }
            (DefaultKind::VALUE(PrismaValue::Bytes(b)), ColumnTypeFamily::Binary) => {
                format!("0x{}", common::format_hex(b)).into()
            }
            (DefaultKind::NOW, ColumnTypeFamily::DateTime) => "CURRENT_TIMESTAMP".into(),
            (DefaultKind::NOW, _) => unreachable!("NOW default on non-datetime column"),
            (DefaultKind::VALUE(val), ColumnTypeFamily::DateTime) => format!("'{}'", val).into(),
            (DefaultKind::VALUE(PrismaValue::String(val)), ColumnTypeFamily::Json) => format!("'{}'", val).into(),
            (DefaultKind::VALUE(PrismaValue::Boolean(val)), ColumnTypeFamily::Boolean) => {
                Cow::from(if *val { "1" } else { "0" })
            }
            (DefaultKind::VALUE(val), _) => val.to_string().into(),
            (DefaultKind::SEQUENCE(_), _) => "".into(),
        }
    }

    fn render_alter_index(&self, indexes: Pair<&IndexWalker<'_>>) -> Vec<String> {
        let index_with_table = format!(
            "{}.{}.{}",
            self.schema_name(),
            indexes.previous().table().name(),
            indexes.previous().name()
        );

        vec![format!(
            "EXEC SP_RENAME N'{index_with_table}', N'{index_new_name}', N'INDEX'",
            index_with_table = index_with_table,
            index_new_name = indexes.next().name(),
        )]
    }

    fn render_create_enum(&self, _: &EnumWalker<'_>) -> Vec<String> {
        unreachable!("render_create_enum on Microsoft SQL Server")
    }

    fn render_create_index(&self, index: &IndexWalker<'_>) -> String {
        let index_type = match index.index_type() {
            IndexType::Unique => "UNIQUE ",
            IndexType::Normal => "",
        };

        let index_name = index.name().replace('.', "_");
        let index_name = self.quote(&index_name);
        let table_reference = self.quote_with_schema(index.table().name()).to_string();

        let columns = index.columns().map(|c| self.quote(c.name()));

        format!(
            "CREATE {index_type}INDEX {index_name} ON {table_reference}({columns})",
            index_type = index_type,
            index_name = index_name,
            table_reference = table_reference,
            columns = columns.join(", "),
        )
    }

    fn render_create_table_as(&self, table: &TableWalker<'_>, table_name: &str) -> String {
        let columns: String = table.columns().map(|column| self.render_column(&column)).join(",\n");

        let primary_columns = table.primary_key_column_names();

        let primary_key = if let Some(primary_columns) = primary_columns.as_ref().filter(|cols| !cols.is_empty()) {
            let index_name = format!("PK_{}_{}", table.name(), primary_columns.iter().join("_"));
            let column_names = primary_columns.iter().map(|col| self.quote(&col)).join(",");

            format!(
                ",\nCONSTRAINT {} PRIMARY KEY ({})",
                self.quote(&index_name),
                column_names
            )
        } else {
            String::new()
        };

        let constraints = table
            .indexes()
            .filter(|index| index.index_type().is_unique())
            .collect::<Vec<_>>();

        let constraints = if !constraints.is_empty() {
            let constraints = constraints
                .iter()
                .map(|index| {
                    let name = index.name().replace('.', "_");
                    let columns = index.columns().map(|col| self.quote(col.name()));

                    format!("CONSTRAINT {} UNIQUE ({})", self.quote(&name), columns.join(","))
                })
                .join(",\n");

            format!(",\n{}", constraints)
        } else {
            String::new()
        };

        format!(
            "CREATE TABLE {} ({columns}{primary_key}{constraints})",
            table_name = self.quote_with_schema(table_name),
            columns = columns,
            primary_key = primary_key,
            constraints = constraints,
        )
    }

    fn render_drop_enum(&self, _: &EnumWalker<'_>) -> Vec<String> {
        unreachable!("render_drop_enum on MSSQL")
    }

    fn render_drop_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        format!(
            "ALTER TABLE {table} DROP CONSTRAINT {constraint_name}",
            table = self.quote_with_schema(foreign_key.table().name()),
            constraint_name = Quoted::mssql_ident(foreign_key.constraint_name().unwrap()),
        )
    }

    fn render_drop_index(&self, index: &IndexWalker<'_>) -> String {
        match index.index_type() {
            IndexType::Normal => format!(
                "DROP INDEX {} ON {}",
                self.quote(index.name()),
                self.quote_with_schema(index.table().name())
            ),
            IndexType::Unique => format!(
                "ALTER TABLE {} DROP CONSTRAINT {}",
                self.quote_with_schema(index.table().name()),
                self.quote(index.name()),
            ),
        }
    }

    fn render_redefine_tables(&self, tables: &[RedefineTable], schemas: &Pair<&SqlSchema>) -> Vec<String> {
        let mut result = Vec::new();

        // All needs to be inside a transaction.
        result.push(r#"BEGIN TRANSACTION"#.to_string());

        for redefine_table in tables {
            let tables = schemas.tables(&redefine_table.table_index);
            // This is a copy of our new modified table.
            let temporary_table_name = format!("_prisma_new_{}", &tables.next().name());

            // If any of the columns is an identity, we should know about it.
            let needs_autoincrement = redefine_table
                .column_pairs
                .iter()
                .any(|(column_indexes, _, _)| tables.columns(column_indexes).next().is_autoincrement());

            // Let's make the [columns] nicely rendered.
            let columns: Vec<_> = redefine_table
                .column_pairs
                .iter()
                .map(|(column_indexes, _, _)| tables.columns(column_indexes).next().name())
                .map(|c| self.quote(c))
                .map(|c| format!("{}", c))
                .collect();

            // We must drop foreign keys pointing to this table before removing
            // any of the table constraints.
            for fk in tables.previous().referencing_foreign_keys() {
                result.push(self.render_drop_foreign_key(&fk));
            }

            // Then the indices...
            for index in tables.previous().indexes() {
                result.push(self.render_drop_index(&index));
            }

            // Remove all constraints from our original table. This will allow
            // us to reuse the same constraint names when creating the temporary
            // table.
            result.push(formatdoc! {r#"
                DECLARE @SQL NVARCHAR(MAX) = N''
                SELECT @SQL += N'ALTER TABLE '
                    + QUOTENAME(OBJECT_SCHEMA_NAME(PARENT_OBJECT_ID))
                    + '.'
                    + QUOTENAME(OBJECT_NAME(PARENT_OBJECT_ID))
                    + ' DROP CONSTRAINT '
                    + OBJECT_NAME(OBJECT_ID) + ';'
                FROM SYS.OBJECTS
                WHERE TYPE_DESC LIKE '%CONSTRAINT'
                    AND OBJECT_NAME(PARENT_OBJECT_ID) = '{table}'
                    AND SCHEMA_NAME(SCHEMA_ID) = '{schema}'
                EXEC sp_executesql @SQL
            "#, table = tables.previous().name(), schema = self.schema_name()});

            // Create the new table.
            result.push(self.render_create_table_as(tables.next(), &temporary_table_name));

            // We cannot insert into autoincrement columns by default. If we
            // have `IDENTITY` in any of the columns, we'll allow inserting
            // momentarily.
            if needs_autoincrement {
                result.push(format!(
                    r#"SET IDENTITY_INSERT {} ON"#,
                    self.quote_with_schema(&temporary_table_name)
                ));
            }

            // Now we copy everything from the old table to the newly created.
            result.push(formatdoc! {r#"
                IF EXISTS(SELECT * FROM {table})
                    EXEC('INSERT INTO {tmp_table} ({columns}) SELECT {columns} FROM {table} WITH (holdlock tablockx)')"#,
                columns = columns.join(","),
                table = self.quote_with_schema(tables.previous().name()),
                tmp_table = self.quote_with_schema(&temporary_table_name),
            });

            // When done copying, disallow identity inserts again if needed.
            if needs_autoincrement {
                result.push(format!(
                    r#"SET IDENTITY_INSERT {} OFF"#,
                    self.quote_with_schema(&temporary_table_name)
                ));
            }

            // Drop the old, now empty table.
            result.extend(self.render_drop_table(tables.previous().name()));

            // Rename the temporary table with the name defined in the migration.
            result.push(self.render_rename_table(&temporary_table_name, tables.next().name()));

            // Recreating all foreign keys pointing to this table
            for fk in tables.next().referencing_foreign_keys() {
                result.push(self.render_add_foreign_key(&fk));
            }

            // Then the indices...
            for index in tables.next().indexes() {
                result.push(self.render_create_index(&index));
            }
        }

        result.push("COMMIT".to_string());

        result
    }

    fn render_rename_table(&self, name: &str, new_name: &str) -> String {
        let with_schema = format!("{}.{}", self.schema_name(), name);

        format!(
            "EXEC SP_RENAME N{}, N{}",
            Quoted::Single(with_schema),
            Quoted::Single(new_name),
        )
    }

    fn render_add_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String {
        let mut add_constraint = String::with_capacity(120);

        write!(
            add_constraint,
            "ALTER TABLE {table} ADD ",
            table = self.quote_with_schema(foreign_key.table().name())
        )
        .unwrap();

        if let Some(constraint_name) = foreign_key.constraint_name() {
            write!(add_constraint, "CONSTRAINT {} ", self.quote(constraint_name)).unwrap();
        }

        write!(
            add_constraint,
            "FOREIGN KEY ({})",
            foreign_key
                .constrained_column_names()
                .iter()
                .map(|col| self.quote(col))
                .join(", ")
        )
        .unwrap();

        add_constraint.push_str(&self.render_references(foreign_key));

        add_constraint
    }

    fn render_drop_table(&self, table_name: &str) -> Vec<String> {
        vec![format!("DROP TABLE {}", self.quote_with_schema(&table_name))]
    }
}

fn render_column_type(column: &ColumnWalker<'_>) -> Cow<'static, str> {
    if !column.column_type().full_data_type.is_empty() {
        return column.column_type().full_data_type.clone().into();
    }

    let r#type = match &column.column_type().family {
        ColumnTypeFamily::Boolean => "bit",
        ColumnTypeFamily::DateTime => "datetime2",
        ColumnTypeFamily::Float => "decimal(32,16)",
        ColumnTypeFamily::Decimal => "decimal(32,16)",
        ColumnTypeFamily::Int => "int",
        ColumnTypeFamily::BigInt => "bigint",
        ColumnTypeFamily::String | ColumnTypeFamily::Json => "nvarchar(1000)",
        ColumnTypeFamily::Binary => "varbinary(max)",
        ColumnTypeFamily::Enum(_) => unimplemented!("Enum not handled yet"),
        ColumnTypeFamily::Uuid => unimplemented!("Uuid not handled yet"),
        ColumnTypeFamily::Unsupported(x) => unimplemented!("{} not handled yet", x),
    };

    r#type.into()
}

fn escape_string_literal(s: &str) -> String {
    s.replace('\'', "''")
}
