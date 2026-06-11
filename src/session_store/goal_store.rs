//! Durable goal store — CRUD methods for goal_runs and goal_steps.

use rusqlite::{params, Result as SqlResult};

use super::records::{GoalRunRecord, GoalStepInsert, GoalStepRecord};
use super::SessionStore;

impl SessionStore {
    pub fn create_goal_run(
        &self,
        id: &str,
        session_id: &str,
        objective: &str,
        status: &str,
        stop_rules_json: &str,
        budget_json: &str,
    ) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO goal_runs (id, session_id, objective, status, stop_rules_json, budget_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, session_id, objective, status, stop_rules_json, budget_json],
        )?;
        Ok(())
    }

    pub fn get_active_goal_run(&self, session_id: &str) -> SqlResult<Option<GoalRunRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, objective, status, stop_rules_json, budget_json,
                    turn_count, last_closeout_status, last_blocker, created_at, updated_at
             FROM goal_runs
             WHERE session_id = ?1 AND status = 'active'
             ORDER BY created_at DESC
             LIMIT 1",
            params![session_id],
            |row| {
                Ok(GoalRunRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    objective: row.get(2)?,
                    status: row.get(3)?,
                    stop_rules_json: row.get(4)?,
                    budget_json: row.get(5)?,
                    turn_count: row.get(6)?,
                    last_closeout_status: row.get(7)?,
                    last_blocker: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_goal_run(&self, goal_id: &str) -> SqlResult<Option<GoalRunRecord>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, session_id, objective, status, stop_rules_json, budget_json,
                    turn_count, last_closeout_status, last_blocker, created_at, updated_at
             FROM goal_runs
             WHERE id = ?1",
            params![goal_id],
            |row| {
                Ok(GoalRunRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    objective: row.get(2)?,
                    status: row.get(3)?,
                    stop_rules_json: row.get(4)?,
                    budget_json: row.get(5)?,
                    turn_count: row.get(6)?,
                    last_closeout_status: row.get(7)?,
                    last_blocker: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        );
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_goal_run_status(
        &self,
        goal_id: &str,
        status: &str,
        last_closeout_status: Option<&str>,
        last_blocker: Option<&str>,
    ) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE goal_runs
             SET status = ?1,
                 last_closeout_status = ?2,
                 last_blocker = ?3,
                 turn_count = turn_count + 1,
                 updated_at = datetime('now')
             WHERE id = ?4",
            params![status, last_closeout_status, last_blocker, goal_id],
        )?;
        Ok(())
    }

    pub fn record_goal_step(&self, insert: &GoalStepInsert) -> SqlResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO goal_steps (id, goal_id, session_id, turn_index, prompt,
                                    closeout_status, verification_status,
                                    changed_files, validation_items,
                                    decision, summary, score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                insert.id,
                insert.goal_id,
                insert.session_id,
                insert.turn_index,
                insert.prompt,
                insert.closeout_status,
                insert.verification_status,
                insert.changed_files,
                insert.validation_items,
                insert.decision,
                insert.summary,
                insert.score,
            ],
        )?;
        Ok(())
    }

    pub fn list_goal_steps(&self, goal_id: &str, limit: i64) -> SqlResult<Vec<GoalStepRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, goal_id, session_id, turn_index, prompt,
                    closeout_status, verification_status,
                    changed_files, validation_items,
                    decision, summary, score, created_at
             FROM goal_steps
             WHERE goal_id = ?1
             ORDER BY turn_index ASC
             LIMIT ?2",
        )?;
        let steps = stmt.query_map(params![goal_id, limit], |row| {
            Ok(GoalStepRecord {
                id: row.get(0)?,
                goal_id: row.get(1)?,
                session_id: row.get(2)?,
                turn_index: row.get(3)?,
                prompt: row.get(4)?,
                closeout_status: row.get(5)?,
                verification_status: row.get(6)?,
                changed_files: row.get(7)?,
                validation_items: row.get(8)?,
                decision: row.get(9)?,
                summary: row.get(10)?,
                score: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;
        steps.collect()
    }
}
