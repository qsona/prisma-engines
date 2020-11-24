//! Render SQL DDL statements.
//!
//! Conventions:
//!
//! - Use 4 spaces for indentation (see common::SQL_INDENTATION)
//! - SQL types and keywords, like CREATE TABLE and VARCHAR, should be upper
//!   case, for consistency.
//! - SqlRenderer implementations do not add semicolons at the end of
//!   statements, this is done later.

mod common;
mod mssql_renderer;
mod mysql_renderer;
mod postgres_renderer;
mod sqlite_renderer;

pub(crate) use common::IteratorJoin;

use crate::{
    pair::Pair,
    sql_migration::{AlterEnum, AlterTable, RedefineTable},
};
use common::{Quoted, QuotedWithSchema};
use sql_schema_describer::{
    walkers::EnumWalker,
    walkers::ForeignKeyWalker,
    walkers::IndexWalker,
    walkers::{ColumnWalker, TableWalker},
    ColumnTypeFamily, DefaultValue, SqlSchema,
};
use std::borrow::Cow;

pub(crate) trait SqlRenderer {
    fn quote<'a>(&self, name: &'a str) -> Quoted<&'a str>;

    fn render_add_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String;

    fn render_alter_enum(&self, alter_enum: &AlterEnum, schemas: &Pair<&SqlSchema>) -> Vec<String>;

    fn render_column(&self, column: &ColumnWalker<'_>) -> String;

    fn render_references(&self, foreign_key: &ForeignKeyWalker<'_>) -> String;

    fn render_default<'a>(&self, default: &'a DefaultValue, family: &ColumnTypeFamily) -> Cow<'a, str>;

    fn render_alter_index(&self, _indexes: Pair<&IndexWalker<'_>>) -> Vec<String> {
        unreachable!("unreachable render_alter_index")
    }

    fn render_alter_table(&self, alter_table: &AlterTable, schemas: &Pair<&SqlSchema>) -> Vec<String>;

    /// Render a `CreateEnum` step.
    fn render_create_enum(&self, create_enum: &EnumWalker<'_>) -> Vec<String>;

    fn render_create_index(&self, index: &IndexWalker<'_>) -> String;

    /// Render a table creation step.
    fn render_create_table(&self, table: &TableWalker<'_>) -> String {
        self.render_create_table_as(table, table.name())
    }

    /// Render a table creation with the provided table name.
    fn render_create_table_as(&self, table: &TableWalker<'_>, table_name: &str) -> String;

    fn render_drop_and_recreate_index(&self, _indexes: Pair<&IndexWalker<'_>>) -> Vec<String> {
        unreachable!("unreachable render_drop_and_recreate_index")
    }

    /// Render a `DropEnum` step.
    fn render_drop_enum(&self, dropped_enum: &EnumWalker<'_>) -> Vec<String>;

    /// Render a `DropForeignKey` step.
    fn render_drop_foreign_key(&self, foreign_key: &ForeignKeyWalker<'_>) -> String;

    /// Render a `DropIndex` step.
    fn render_drop_index(&self, index: &IndexWalker<'_>) -> String;

    /// Render a `DropTable` step.
    fn render_drop_table(&self, table_name: &str) -> Vec<String> {
        vec![format!("DROP TABLE {}", self.quote(&table_name))]
    }

    /// Render a `RedefineTables` step.
    fn render_redefine_tables(&self, tables: &[RedefineTable], schemas: &Pair<&SqlSchema>) -> Vec<String>;

    /// Render a table renaming step.
    fn render_rename_table(&self, name: &str, new_name: &str) -> String;
}
