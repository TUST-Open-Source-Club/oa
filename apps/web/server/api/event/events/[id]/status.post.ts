/** 切换活动状态，并在报名截止/结束时自动补建收尾任务（幂等）。 */
import { eventStatusTasks } from '~/utils/event-status-tasks'

export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  const id = getRouterParam(event, 'id') ?? ''
  const body = (await readBody(event)) as { status?: string }
  const status = body.status ?? ''
  if (!['draft', 'open', 'closed', 'finished'].includes(status)) {
    throw createError({ statusCode: 422, statusMessage: 'EVENT_STATUS_INVALID' })
  }
  const config = useRuntimeConfig(event)
  const headers = { authorization }

  type EventDetail = { id: string; status: string; taskBoardId: string | null }
  const updated = (await $fetch(`${config.eventServiceUrl}/api/v1/event/events/${id}`, {
    method: 'PATCH',
    headers,
    body: { status },
  })) as EventDetail

  const wanted = eventStatusTasks(status)
  if (!wanted.length || !updated.taskBoardId) {
    return { event: updated, added: 0 }
  }

  const taskBase = config.taskServiceUrl
  const columns = (await $fetch(
    `${taskBase}/api/v1/task/boards/${updated.taskBoardId}/columns`,
    { headers },
  )) as Array<{ id: string }>
  const columnId = columns[0]?.id
  if (!columnId) return { event: updated, added: 0 }

  const existing = (await $fetch(
    `${taskBase}/api/v1/task/boards/${updated.taskBoardId}/tasks`,
    { headers },
  )) as Array<{ title: string }>
  const titles = new Set(existing.map((task) => task.title))

  let added = 0
  for (const task of wanted) {
    if (titles.has(task.title)) continue
    await $fetch(`${taskBase}/api/v1/task/columns/${columnId}/tasks`, {
      method: 'POST',
      headers,
      body: {
        title: task.title,
        descriptionMd: task.descriptionMd,
        priority: task.priority,
        eventId: id,
      },
    })
    added += 1
  }
  return { event: updated, added }
})
