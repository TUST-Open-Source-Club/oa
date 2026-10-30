/** 任务视图的纯逻辑（截止分组）。 */

/** 截止分组。 */
export type DueBucket = 'overdue' | 'today' | 'week' | 'later' | 'none'

/** 单个任务的截止分组。 */
export function dueBucket(dueAt: string | null | undefined, now: Date = new Date()): DueBucket {
  if (!dueAt) return 'none'
  const due = new Date(dueAt)
  if (Number.isNaN(due.getTime())) return 'none'
  if (due.getTime() < now.getTime()) return 'overdue'
  const sameDay =
    due.getFullYear() === now.getFullYear() &&
    due.getMonth() === now.getMonth() &&
    due.getDate() === now.getDate()
  if (sameDay) return 'today'
  const weekLater = new Date(now.getTime() + 7 * 24 * 3600 * 1000)
  if (due.getTime() <= weekLater.getTime()) return 'week'
  return 'later'
}

/** 按截止时间分组（固定顺序）。 */
export function groupTasksByDue<T extends { dueAt?: string | null }>(
  tasks: T[],
  now: Date = new Date(),
): Record<DueBucket, T[]> {
  const groups: Record<DueBucket, T[]> = {
    overdue: [],
    today: [],
    week: [],
    later: [],
    none: [],
  }
  for (const task of tasks) {
    groups[dueBucket(task.dueAt, now)].push(task)
  }
  return groups
}
