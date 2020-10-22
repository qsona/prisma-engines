use crate::read_datamodel_from_file;
use colored::*;
use migration_core::{
    commands::{DiagnoseMigrationHistoryInput, DiagnoseMigrationHistoryOutput, HistoryDiagnostic},
    GateKeeper,
};

#[derive(structopt::StructOpt)]
pub(crate) struct ApplyMigrations {
    #[structopt(long)]
    force: bool,

    #[structopt(default_value = "prisma/migrations")]
    migrations_directory_path: String,

    #[structopt(default_value = "prisma/schema.prisma")]
    prisma_schema_path: String,
}

impl ApplyMigrations {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let prisma_schema = read_datamodel_from_file(&self.prisma_schema_path)?;

        let api = migration_core::migration_api(&prisma_schema, GateKeeper::allow_all_whitelist()).await?;

        let DiagnoseMigrationHistoryOutput {
            drift,
            history,
            failed_migration_names,
            edited_migration_names,
        } = api
            .diagnose_migration_history(&DiagnoseMigrationHistoryInput {
                migrations_directory_path: self.migrations_directory_path,
            })
            .await?;

        if let Some(drift) = drift {
            eprintln!("{} {:?}", "Drift detected!".red().bold(), drift);

            if !self.force {
                return Ok(());
            }
        }

        match history {
            Some(HistoryDiagnostic::DatabaseIsBehind { .. }) | None => (),
            Some(diagnostic) => eprintln!("{} {:?}", "History problem!".red().bold(), diagnostic),
        }

        Ok(())
    }
}
