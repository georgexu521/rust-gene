//! v18 migration — add score column to goal_steps for scored eval tracking.

use rusqlite::{Connection, Result as SqlResult};

use crate::migrations::framework::add_column_if_missing;

pub struct V18AddGoalStepScore;

impl crate::migrations::Migration for V18AddGoalStepScore {
    fn version(&self) -> i32 {
        18
    }

    fn name(&self) -> &str {
        "v18_add_goal_step_score"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        add_column_if_missing(conn, "goal_steps", "score", "REAL")
    }
}
