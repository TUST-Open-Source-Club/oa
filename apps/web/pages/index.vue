<script setup lang="ts">
/**
 * 工作台：模块入口 + 账号 / 通知概览。
 *
 * 只展示真实数据（当前用户、未读通知数），不构造假指标；模块说明用于降低新成员上手成本。
 */
import { Button } from '@club-oa/ui'

const auth = useAuthStore()
const notifications = useNotificationsStore()
const router = useRouter()

/** 模块入口：与侧栏同一组模块，附一句用途说明。 */
const modules = [
  { to: '/im', label: '消息', description: '群聊与私聊、实时沟通与提醒', icon: 'chat' },
  { to: '/tasks', label: '任务', description: '看板协作、指派与截止时间', icon: 'board' },
  { to: '/docs', label: '文档', description: '知识库、页面协作与版本记录', icon: 'doc' },
  { to: '/events', label: '活动', description: '报名表单、名单审核与签到统计', icon: 'calendar' },
  { to: '/drive', label: '网盘', description: '资料归档、分享与在线预览', icon: 'folder' },
  { to: '/notifications', label: '通知', description: '任务、活动与系统消息汇总', icon: 'bell' },
]

/** 角色英文标识 → 中文名。 */
const roleLabels: Record<string, string> = {
  superadmin: '超级管理员',
  admin: '管理员',
  member: '成员',
}

const roles = computed(() => (auth.user?.roles ?? []).map((role) => roleLabels[role] ?? role))

/** 今天日期（如「10月28日 星期二」）。 */
const today = computed(() =>
  new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }).format(new Date()),
)

/** 按当前时段给出问候语。 */
const greeting = computed(() => {
  const hour = new Date().getHours()
  if (hour < 6) return '夜深了'
  if (hour < 12) return '早上好'
  if (hour < 18) return '下午好'
  return '晚上好'
})

onMounted(() => {
  void notifications.fetchUnreadCount().catch(() => undefined)
})
</script>

<template>
  <div class="mx-auto w-full max-w-[1040px]">
    <!-- 页头 -->
    <header class="flex flex-wrap items-end justify-between gap-4">
      <div>
        <p class="text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">{{ today }}</p>
        <h1 class="mt-1.5 text-[22px] font-semibold tracking-[-0.02em] text-[var(--text-strong)]">
          {{ greeting }}，{{ auth.user?.nickname }}
        </h1>
        <p class="mt-1 text-[13px] text-neutral-500">从一个模块开始，或先看看有没有新的通知。</p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="secondary" @click="router.push('/notifications')">
          <AppIcon name="inbox" class="size-4" />
          通知
          <span
            v-if="notifications.unreadCount > 0"
            class="rounded-full bg-danger-500 px-1.5 text-[10px] leading-4 font-medium text-white"
          >
            {{ notifications.unreadCount > 99 ? '99+' : notifications.unreadCount }}
          </span>
        </Button>
        <Button @click="router.push('/im')">
          <AppIcon name="chat" class="size-4" />
          进入消息
        </Button>
      </div>
    </header>

    <div class="mt-7 grid gap-6 lg:grid-cols-[minmax(0,1fr)_264px]">
      <!-- 模块列表 -->
      <section
        class="overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
      >
        <div class="flex items-center justify-between border-b border-[var(--border)] px-4 py-2.5">
          <p class="text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">模块</p>
          <p class="text-[11px] text-neutral-400">{{ modules.length }} 个入口</p>
        </div>
        <NuxtLink
          v-for="(item, index) in modules"
          :key="item.to"
          :to="item.to"
          class="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-[var(--surface-hover)]"
          :class="index > 0 ? 'border-t border-[var(--border)]' : ''"
        >
          <span
            class="flex size-8 items-center justify-center rounded-md border border-[var(--border)] bg-[var(--surface-muted)] text-neutral-500 transition group-hover:border-[var(--border-strong)] group-hover:text-primary-600"
          >
            <AppIcon :name="item.icon" class="size-4" />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block text-[13px] font-medium text-[var(--text-strong)]">{{ item.label }}</span>
            <span class="mt-0.5 block truncate text-[12px] text-neutral-400">{{ item.description }}</span>
          </span>
          <AppIcon
            name="arrowRight"
            class="size-4 shrink-0 text-neutral-300 transition group-hover:translate-x-0.5 group-hover:text-neutral-500"
          />
        </NuxtLink>
      </section>

      <!-- 右侧概览 -->
      <aside class="space-y-4">
        <section
          class="rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]"
        >
          <p class="text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">账号</p>
          <div class="mt-3 flex items-center gap-3">
            <UserAvatar :name="auth.user?.nickname" :seed="auth.user?.id" size="lg" />
            <div class="min-w-0">
              <p class="truncate text-[13px] font-medium text-[var(--text-strong)]">
                {{ auth.user?.nickname }}
              </p>
              <p class="truncate text-[12px] text-neutral-400">{{ auth.user?.email }}</p>
            </div>
          </div>
          <div v-if="roles.length" class="mt-3 flex flex-wrap gap-1.5">
            <span
              v-for="role in roles"
              :key="role"
              class="rounded-full border border-[var(--border)] bg-[var(--surface-muted)] px-2 py-0.5 text-[11px] text-neutral-500"
            >
              {{ role }}
            </span>
          </div>
          <NuxtLink
            to="/settings"
            class="mt-3 inline-flex items-center gap-1 text-[12px] font-medium text-primary-600 hover:text-primary-700"
          >
            账号设置
            <AppIcon name="arrowRight" class="size-3.5" />
          </NuxtLink>
        </section>

        <section
          class="rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]"
        >
          <div class="flex items-center justify-between">
            <p class="text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">通知</p>
            <span
              v-if="notifications.unreadCount > 0"
              class="rounded-full bg-danger-50 px-1.5 text-[11px] font-medium text-danger-700"
            >
              {{ notifications.unreadCount }} 未读
            </span>
          </div>
          <p class="mt-2 text-[12px] leading-relaxed text-neutral-500">
            {{
              notifications.unreadCount > 0
                ? '有未读通知，点开可以查看任务、活动与文档的变更。'
                : '暂时没有未读通知。'
            }}
          </p>
          <NuxtLink
            to="/notifications"
            class="mt-3 inline-flex items-center gap-1 text-[12px] font-medium text-primary-600 hover:text-primary-700"
          >
            查看全部
            <AppIcon name="arrowRight" class="size-3.5" />
          </NuxtLink>
        </section>
      </aside>
    </div>
  </div>
</template>
