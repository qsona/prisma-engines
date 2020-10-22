use super::MigrationCommand;
use crate::{migration_engine::MigrationEngine, CoreResult};
use serde::{Deserialize, Serialize};

/// The input to the MarkMigrationRolledBack command.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkMigrationRolledBackInput {
    /// The full name of the migration to mark as rolled back.
    pub migration_name: String,
}

/// The output of the MarkMigrationRolledBack command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkMigrationRolledBackOutput {}

/// Mark a migration as rolled back.
#[derive(Debug)]
pub struct MarkMigrationRolledBackCommand;

#[async_trait::async_trait]
impl MigrationCommand for MarkMigrationRolledBackCommand {
    type Input = MarkMigrationRolledBackInput;
    type Output = MarkMigrationRolledBackOutput;

    async fn execute<C, D>(_input: &Self::Input, _engine: &MigrationEngine<C, D>) -> CoreResult<Self::Output>
    where
        C: migration_connector::MigrationConnector<DatabaseMigration = D>,
        D: migration_connector::DatabaseMigrationMarker + Send + Sync + 'static,
    {
        todo!()
    }
}
