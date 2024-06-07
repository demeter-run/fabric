use anyhow::Result;
use std::{path::Path, sync::Arc};

use crate::domain::management;
use crate::domain::management::project::Project;
use crate::driven::sqlite::{project::SqliteProjectState, SqliteState};

pub async fn server() -> Result<()> {
    let sqlite = Arc::new(SqliteState::new(Path::new("dev.db")).await?);
    sqlite.migrate().await?;

    let project_state = Arc::new(SqliteProjectState::new(sqlite));

    let project = Project {
        name: "test name".into(),
        slug: "test-slug-2".into(),
        description: "test description".into(),
    };

    management::project::create(project_state, project).await?;

    Ok(())
}

