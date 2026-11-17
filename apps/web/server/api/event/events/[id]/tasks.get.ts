/** 活动筹备任务聚合：读活动关联看板的卡片并给出进度。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  const id = getRouterParam(event, 'id') ?? ''
  const config = useRuntimeConfig(event)
  const headers = { authorization }

  type EventDetail = { taskProjectId: string | null; taskBoardId: string | null }
  const detail = (await $fetch(
    `${config.eventServiceUrl}/api/v1/event/events/${id}`,
    { headers },
  )) as EventDetail
  if (!detail.taskBoardId) {
    return { boardId: null, projectId: null, tasks: [], progress: { total: 0, done: 0 } }
  }
  const tasks = (await $fetch(
    `${config.taskServiceUrl}/api/v1/task/boards/${detail.taskBoardId}/tasks`,
    { headers },
  )) as Array<{ id: string; title: string; status: string; dueAt: string | null; assigneeIds: string[] }>
  const done = tasks.filter((task) => task.status === 'done').length
  return {
    boardId: detail.taskBoardId,
    projectId: detail.taskProjectId,
    tasks,
    progress: { total: tasks.length, done },
  }
})
