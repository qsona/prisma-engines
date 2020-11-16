use super::SqlSchemaDifferFlavour;
use crate::sql_schema_differ::column::ColumnDiffer;
use crate::sql_schema_differ::column::ColumnTypeChange;
use crate::{flavour::MssqlFlavour, sql_schema_differ::SqlSchemaDiffer};
use sql_schema_describer::walkers::IndexWalker;
use sql_schema_describer::ColumnTypeFamily;
use std::collections::HashSet;

impl SqlSchemaDifferFlavour for MssqlFlavour {
    fn should_skip_index_for_new_table(&self, index: &IndexWalker<'_>) -> bool {
        index.index_type().is_unique()
    }

    fn tables_to_redefine(&self, differ: &SqlSchemaDiffer<'_>) -> HashSet<String> {
        differ
            .table_pairs()
            .filter(|differ| differ.column_pairs().any(|c| c.autoincrement_changed()))
            .map(|table| table.next().name().to_owned())
            .collect()
    }

    fn column_type_change(&self, differ: &ColumnDiffer<'_>) -> Option<ColumnTypeChange> {
        if differ.previous.column_type_family() == differ.next.column_type_family() {
            return None;
        }

        match (differ.previous.column_type_family(), differ.next.column_type_family()) {
            (_, ColumnTypeFamily::String) => Some(ColumnTypeChange::SafeCast),
            (ColumnTypeFamily::String, ColumnTypeFamily::Int)
            | (ColumnTypeFamily::DateTime, ColumnTypeFamily::Float)
            | (ColumnTypeFamily::String, ColumnTypeFamily::Float) => Some(ColumnTypeChange::NotCastable),
            (_, _) => Some(ColumnTypeChange::RiskyCast),
        }
    }
}
