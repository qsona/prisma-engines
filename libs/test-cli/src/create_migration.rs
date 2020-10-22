use colored::*;
use migration_core::{commands::CreateMigrationInput, GateKeeper};

use crate::read_datamodel_from_file;

#[derive(Debug, structopt::StructOpt)]
pub(crate) struct CreateMigration {
    #[structopt(default_value = "prisma/migrations")]
    migrations_directory_path: String,

    #[structopt(long)]
    migration_name: String,

    #[structopt(default_value = "prisma/schema.prisma")]
    prisma_schema_path: String,

    #[structopt(long)]
    draft: bool,
}

impl CreateMigration {
    #[tracing::instrument]
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let prisma_schema = read_datamodel_from_file(&self.prisma_schema_path)?;

        let api = migration_core::migration_api(&prisma_schema, GateKeeper::allow_all_whitelist()).await?;

        let input = CreateMigrationInput {
            migrations_directory_path: self.migrations_directory_path,
            prisma_schema,
            migration_name: self.migration_name,
            draft: self.draft,
        };

        let response = api.create_migration(&input).await?;

        if let Some(migration_name) = &response.generated_migration_name {
            eprintln!(
                "{} `{}`",
                "Generated a new migration in".green().bold(),
                migration_name.yellow()
            );
        } else {
            eprintln!(
                "{}",
                "The migrations are up-to-date with the prisma schema.".green().bold()
            );
        }

        Ok(())
    }
}
