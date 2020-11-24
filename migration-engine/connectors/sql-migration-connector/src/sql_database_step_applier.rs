use crate::{
    pair::Pair,
    sql_migration::{CreateTable, DropTable, SqlMigration, SqlMigrationStep},
    SqlFlavour, SqlMigrationConnector,
};
use migration_connector::{
    ConnectorResult, DatabaseMigrationMarker, DatabaseMigrationStepApplier, DestructiveChangeDiagnostics,
    PrettyDatabaseMigrationStep,
};
use sql_schema_describer::{walkers::SqlSchemaExt, SqlSchema};

#[async_trait::async_trait]
impl DatabaseMigrationStepApplier<SqlMigration> for SqlMigrationConnector {
    #[tracing::instrument(skip(self, database_migration))]
    async fn apply_step(&self, database_migration: &SqlMigration, index: usize) -> ConnectorResult<bool> {
        self.apply_next_step(
            &database_migration.steps,
            index,
            self.flavour(),
            database_migration.schemas(),
        )
        .await
    }

    fn render_steps_pretty(
        &self,
        database_migration: &SqlMigration,
    ) -> ConnectorResult<Vec<PrettyDatabaseMigrationStep>> {
        render_steps_pretty(&database_migration, self.flavour(), database_migration.schemas())
    }

    fn render_script(&self, database_migration: &SqlMigration, diagnostics: &DestructiveChangeDiagnostics) -> String {
        if database_migration.is_empty() {
            return "-- This is an empty migration.".to_string();
        }

        let mut script = String::with_capacity(40 * database_migration.steps.len());

        // Note: it would be much nicer if we could place the warnings next to
        // the SQL for the steps that triggered them.
        if diagnostics.has_warnings() || !diagnostics.unexecutable_migrations.is_empty() {
            script.push_str("/*\n  Warnings:\n\n");

            for warning in &diagnostics.warnings {
                script.push_str("  - ");
                script.push_str(&warning.description);
                script.push('\n');
            }

            for unexecutable in &diagnostics.unexecutable_migrations {
                script.push_str("  - ");
                script.push_str(&unexecutable.description);
                script.push('\n');
            }

            script.push_str("\n*/\n")
        }

        for step in &database_migration.steps {
            let statements: Vec<String> = render_raw_sql(
                step,
                self.flavour(),
                Pair::new(&database_migration.before, &database_migration.after),
            );

            if !statements.is_empty() {
                script.push_str("-- ");
                script.push_str(step.description());
                script.push('\n');

                for statement in statements {
                    script.push_str(&statement);
                    script.push_str(";\n");
                }
            }
        }

        script
    }

    async fn apply_script(&self, script: &str) -> ConnectorResult<()> {
        Ok(self.conn().raw_cmd(script).await?)
    }
}

impl SqlMigrationConnector {
    async fn apply_next_step(
        &self,
        steps: &[SqlMigrationStep],
        index: usize,
        renderer: &(dyn SqlFlavour + Send + Sync),
        schemas: Pair<&SqlSchema>,
    ) -> ConnectorResult<bool> {
        let has_this_one = steps.get(index).is_some();

        if !has_this_one {
            return Ok(false);
        }

        let step = &steps[index];
        tracing::debug!(?step);

        for sql_string in render_raw_sql(&step, renderer, schemas) {
            tracing::debug!(index, %sql_string);
            self.conn().raw_cmd(&sql_string).await?;
        }

        Ok(true)
    }
}

fn render_steps_pretty(
    database_migration: &SqlMigration,
    renderer: &(dyn SqlFlavour + Send + Sync),
    schemas: Pair<&SqlSchema>,
) -> ConnectorResult<Vec<PrettyDatabaseMigrationStep>> {
    let mut steps = Vec::with_capacity(database_migration.steps.len());

    for step in &database_migration.steps {
        let sql = render_raw_sql(&step, renderer, schemas).join(";\n");

        if !sql.is_empty() {
            steps.push(PrettyDatabaseMigrationStep {
                step: serde_json::to_value(&step).unwrap_or_else(|_| serde_json::json!({})),
                raw: sql,
            });
        }
    }

    Ok(steps)
}

fn render_raw_sql(
    step: &SqlMigrationStep,
    renderer: &(dyn SqlFlavour + Send + Sync),
    schemas: Pair<&SqlSchema>,
) -> Vec<String> {
    match step {
        SqlMigrationStep::AlterEnum(alter_enum) => renderer.render_alter_enum(alter_enum, &schemas),
        SqlMigrationStep::RedefineTables(redefine_tables) => renderer.render_redefine_tables(redefine_tables, &schemas),
        SqlMigrationStep::CreateEnum(create_enum) => {
            renderer.render_create_enum(&schemas.next().enum_walker_at(create_enum.enum_index))
        }
        SqlMigrationStep::DropEnum(drop_enum) => {
            renderer.render_drop_enum(&schemas.previous().enum_walker_at(drop_enum.enum_index))
        }
        SqlMigrationStep::CreateTable(CreateTable { table_index }) => {
            let table = schemas.next().table_walker_at(*table_index);

            vec![renderer.render_create_table(&table)]
        }
        SqlMigrationStep::DropTable(DropTable { table_index }) => {
            renderer.render_drop_table(schemas.previous().table_walker_at(*table_index).name())
        }
        SqlMigrationStep::RedefineIndex { table, index } => {
            renderer.render_drop_and_recreate_index(schemas.tables(table).indexes(index).as_ref())
        }
        SqlMigrationStep::AddForeignKey(add_foreign_key) => {
            let foreign_key = schemas
                .next()
                .table_walker_at(add_foreign_key.table_index)
                .foreign_key_at(add_foreign_key.foreign_key_index);

            vec![renderer.render_add_foreign_key(&foreign_key)]
        }
        SqlMigrationStep::DropForeignKey(drop_foreign_key) => {
            let foreign_key = schemas
                .previous()
                .table_walker_at(drop_foreign_key.table_index)
                .foreign_key_at(drop_foreign_key.foreign_key_index);

            vec![renderer.render_drop_foreign_key(&foreign_key)]
        }
        SqlMigrationStep::AlterTable(alter_table) => renderer.render_alter_table(alter_table, &schemas),
        SqlMigrationStep::CreateIndex(create_index) => vec![renderer.render_create_index(
            &schemas
                .next()
                .table_walker_at(create_index.table_index)
                .index_at(create_index.index_index),
        )],
        SqlMigrationStep::DropIndex(drop_index) => vec![renderer.render_drop_index(
            &schemas
                .previous()
                .table_walker_at(drop_index.table_index)
                .index_at(drop_index.index_index),
        )],
        SqlMigrationStep::AlterIndex { table, index } => {
            renderer.render_alter_index(schemas.tables(table).indexes(index).as_ref())
        }
    }
}
