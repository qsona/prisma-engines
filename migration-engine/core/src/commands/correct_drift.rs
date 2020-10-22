use super::MigrationCommand;
use crate::{migration_engine::MigrationEngine, CoreResult};
use serde::{Deserialize, Serialize};

/// The input to the CorrectDrift command.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectDriftInput {
    /// A database script to apply.
    pub script: String,
}

/// The output of the CorrectDrift command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectDriftOutput {}

/// Correct detected drift by applying a suggested or user-defined script.
#[derive(Debug)]
pub struct CorrectDriftCommand;

#[async_trait::async_trait]
impl MigrationCommand for CorrectDriftCommand {
    type Input = CorrectDriftInput;
    type Output = CorrectDriftOutput;

    async fn execute<C, D>(_input: &Self::Input, _engine: &MigrationEngine<C, D>) -> CoreResult<Self::Output>
    where
        C: migration_connector::MigrationConnector<DatabaseMigration = D>,
        D: migration_connector::DatabaseMigrationMarker + Send + Sync + 'static,
    {
        todo!()
    }
}
