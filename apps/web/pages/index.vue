<script setup lang="ts">
/** 工作台：欢迎语 + 模块入口 + 使用提示。 */
const auth = useAuthStore()

const modules = [
  { to: '/im', label: '消息', description: '群聊与私聊、实时沟通', icon: 'chat', tone: 'bg-primary-50 text-primary-700 dark:bg-primary-900/40 dark:text-primary-200' },
  { to: '/tasks', label: '任务', description: '看板协作、指派与截止', icon: 'board', tone: 'bg-info-50 text-info-700 dark:bg-info-500/10' },
  { to: '/docs', label: '文档', description: '知识库、协同编辑', icon: 'doc', tone: 'bg-success-50 text-success-700 dark:bg-success-500/10' },
  { to: '/events', label: '活动', description: '报名、签到与统计', icon: 'calendar', tone: 'bg-warning-50 text-warning-700 dark:bg-warning-500/10' },
  { to: '/drive', label: '网盘', description: '资料归档、分享下载', icon: 'folder', tone: 'bg-neutral-100 text-neutral-600 dark:bg-neutral-800' },
  { to: '/notifications', label: '通知', description: '系统与业务提醒', icon: 'bell', tone: 'bg-danger-50 text-danger-700 dark:bg-danger-500/10' },
]
</script>

<template>
  <div class="space-y-6">
    <div
      class="rounded-[var(--radius-card)] bg-gradient-to-br from-primary-600 to-primary-800 px-6 py-8 text-white shadow-[var(--shadow-card)]"
    >
      <h1 class="text-2xl font-semibold">你好，{{ auth.user?.nickname }}</h1>
      <p class="mt-1.5 text-sm text-primary-100">
        欢迎回到{{ $config.public.appName }}，从下面选择一个模块开始吧。
      </p>
    </div>

    <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
      <NuxtLink
        v-for="item in modules"
        :key="item.to"
        :to="item.to"
        class="group rounded-[var(--radius-card)] border border-neutral-200 bg-white p-5 shadow-[var(--shadow-card)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-pop)] dark:border-neutral-800 dark:bg-neutral-900"
      >
        <span class="flex size-10 items-center justify-center rounded-xl" :class="item.tone">
          <AppIcon :name="item.icon" class="size-5" />
        </span>
        <h2 class="mt-3 text-sm font-semibold">{{ item.label }}</h2>
        <p class="mt-1 text-xs text-neutral-400">{{ item.description }}</p>
      </NuxtLink>
    </div>
  </div>
</template>
