/** 生成活动筹备清单：创建独立项目 + 看板 + 模板卡片，并回写活动。 */
import { buildChecklist } from '~/utils/event-templates'

export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  const id = getRouterParam(event, 'id') ?? ''
  if (!id) throw createError({ statusCode: 400, statusMessage: 'EVENT_ID_REQUIRED' })
  const body = (await readBody(event).catch(() => ({}))) as { templateKey?: string }

  const config = useRuntimeConfig(event)
  const eventBase = config.eventServiceUrl
  const taskBase = config.taskServiceUrl
  const headers = { authorization }

  type EventDetail = {
    id: string
    title: string
    startAt: string | null
    taskProjectId: string | null
    taskBoardId: string | null
  }
  const detail = (await $fetch(`${eventBase}/api/v1/event/events/${id}`, {
    headers,
  })) as EventDetail

  // 幂等：已生成直接返回
  if (detail.taskProjectId && detail.taskBoardId) {
    return {
      projectId: detail.taskProjectId,
      boardId: detail.taskBoardId,
      tasks: 0,
      existed: true,
    }
  }

  const templateKey = body.templateKey ?? 'recruit'
  const project = (await $fetch(`${taskBase}/api/v1/task/projects`, {
    method: 'POST',
    headers,
    body: { name: detail.title, description: `活动筹备：${detail.title}` },
  })) as { id: string }

  const boards = (await $fetch(`${taskBase}/api/v1/task/projects/${project.id}/boards`, {
    headers,
  })) as Array<{ id: string; name: string }>
  const board = boards[0]
  if (!board) throw createError({ statusCode: 502, statusMessage: 'TASK_BOARD_CREATE_FAILED' })
  await $fetch(`${taskBase}/api/v1/task/boards/${board.id}`, {
    method: 'PATCH',
    headers,
    body: { name: detail.title },
  })

  const columns = (await $fetch(`${taskBase}/api/v1/task/boards/${board.id}/columns`, {
    headers,
  })) as Array<{ id: string; name: string }>
  const first = columns[0]
  if (!first) throw createError({ statusCode: 502, statusMessage: 'TASK_COLUMN_MISSING' })

  const items = buildChecklist(templateKey, detail.startAt, new Date())
  const taskIds: string[] = []
  for (const item of items) {
    const task = (await $fetch(`${taskBase}/api/v1/task/columns/${first.id}/tasks`, {
      method: 'POST',
      headers,
      body: {
        title: item.title,
        descriptionMd: item.descriptionMd,
        dueAt: item.dueAt,
        priority: item.priority,
        eventId: id,
      },
    })) as { id: string }
    taskIds.push(task.id)
  }

  await $fetch(`${eventBase}/api/v1/event/events/${id}/checklist`, {
    method: 'PUT',
    headers,
    body: { projectId: project.id, boardId: board.id, templateKey },
  })

  return { projectId: project.id, boardId: board.id, tasks: taskIds.length, existed: false }
})
