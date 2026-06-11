//! v18 migration — add score column to goal_steps for scored eval tracking.

use rusqlite::{Connection, Result as SqlResult};

pub struct V18AddGoalStepScore;

impl crate::migrations::Migration for V18AddGoalStepScore {
    fn version(&self) -> i32 {
        18
    }

    fn name(&self) -> &str {
        "v18_add_goal_step_score"
    }

    fn up(&self, conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(ADD_SCORE_COLUMN)
    }
}

const ADD_SCORE_COLUMN: &str = r#"
ALTER TABLE goal_steps ADD COLUMN score REAL;
"#;
