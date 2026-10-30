//! 到期提醒：扫描 24 小时内到期的任务，给负责人发通知（每天一次）。

use chrono::Duration;
use uuid::Uuid;

use club_common::AppError;

use crate::repo;
use crate::state::SharedState;

/// 扫描并提醒，返回本次提醒的任务数。
pub async fn remind_due_tasks(state: &SharedState) -> Result<u32, AppError> {
    let now = state.now();
    let tasks = repo::due_soon_tasks(&state.db, now, now + Duration::hours(24)).await?;
    let mut reminded = 0u32;
    for task in tasks {
        let targets: Vec<Uuid> = repo::list_task_assignees(&state.db, task.id).await?;
        if !targets.is_empty() {
            crate::routes::enqueue_event(
                state,
                "task.due_soon",
                task.created_by,
                targets,
                task.id,
                &format!("任务即将到期：{}", task.title),
                "请在 24 小时内处理",
                "high",
            )
            .await;
        }
        repo::mark_due_reminded(&state.db, &task, now).await?;
        reminded += 1;
    }
    Ok(reminded)
}
