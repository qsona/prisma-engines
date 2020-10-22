use super::MigrationCommand;
use crate::{migration_engine::MigrationEngine, CoreResult};
use serde::{Deserialize, Serialize};

/// The input to the MarkMigrationApplied command.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkMigrationAppliedInput {
    /// The full name of the migration directory.
    pub migration_name: String,
    /// The path of the migrations directory.
    pub migrations_directory_path: String,
}

/// The output of the MarkMigrationApplied command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkMigrationAppliedOutput {}

/// Mark a migration as applied in the database, without actually applying it.
#[derive(Debug)]
pub struct MarkMigrationAppliedCommand;

#[async_trait::async_trait]
impl MigrationCommand for MarkMigrationAppliedCommand {
    type Input = MarkMigrationAppliedInput;
    type Output = MarkMigrationAppliedOutput;

    async fn execute<C, D>(_input: &Self::Input, _engine: &MigrationEngine<C, D>) -> CoreResult<Self::Output>
    where
        C: migration_connector::MigrationConnector<DatabaseMigration = D>,
        D: migration_connector::DatabaseMigrationMarker + Send + Sync + 'static,
    {
        todo!()
    }
}
