use crate::SqlMigrationStep;
use migration_connector::*;
use prisma_datamodel::Schema;

pub struct SqlDatabaseMigrationStepsInferrer {}

#[allow(unused, dead_code)]
impl DatabaseMigrationStepsInferrer<SqlMigrationStep> for SqlDatabaseMigrationStepsInferrer {
    fn infer(&self, previous: &Schema, next: &Schema, steps: Vec<MigrationStep>) -> Vec<SqlMigrationStep> {
        vec![]
    }
}
