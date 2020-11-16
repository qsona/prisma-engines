mod mssql;
mod mysql;
mod postgres;
mod sqlite;

use super::SqlSchemaCalculator;
use datamodel::walkers::ModelWalker;
use datamodel::{walkers::ScalarFieldWalker, NativeTypeInstance, ScalarType};
use sql_schema_describer as sql;

pub(crate) trait SqlSchemaCalculatorFlavour {
    fn calculate_enums(&self, _calculator: &SqlSchemaCalculator<'_>) -> Vec<sql::Enum> {
        Vec::new()
    }

    fn column_type_for_native_type(
        &self,
        field: &ScalarFieldWalker<'_>,
        scalar_type: ScalarType,
        native_type_instance: &NativeTypeInstance,
    ) -> sql::ColumnType;

    fn m2m_foreign_key_action(&self, _model_a: &ModelWalker<'_>, _model_b: &ModelWalker<'_>) -> sql::ForeignKeyAction {
        sql::ForeignKeyAction::Cascade
    }

    // TODO: Maybe we should rethink this a bit?
    fn single_field_index_name(&self, model_name: &str, field_name: &str) -> String {
        format!("{}.{}_unique", model_name, field_name)
    }
}
