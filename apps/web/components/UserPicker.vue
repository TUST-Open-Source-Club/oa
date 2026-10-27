<script setup lang="ts">
/** 选人组件：单选/多选 + 关键字搜索（防抖），支持按 ID 回填已有选择。 */
import { Input } from '@club-oa/ui'

/** 用户条目。 */
interface UserItem {
  id: string
  username: string
  nickname: string
  department: string | null
}

const props = withDefaults(
  defineProps<{
    modelValue: string | string[] | null
    multiple?: boolean
    placeholder?: string
    excludeIds?: string[]
  }>(),
  { multiple: false, placeholder: '搜索姓名 / 邮箱', excludeIds: () => [] },
)
const emit = defineEmits<{ 'update:modelValue': [value: string | string[] | null] }>()

const auth = useAuthStore()
const keyword = ref('')
const results = ref<UserItem[]>([])
const directory = ref<Record<string, UserItem>>({})
const focused = ref(false)
const loading = ref(false)
const activeIndex = ref(-1)

const selectedIds = computed<string[]>(() =>
  Array.isArray(props.modelValue) ? props.modelValue : props.modelValue ? [props.modelValue] : [],
)
const selected = computed(() => selectedIds.value.map((id) => directory.value[id]).filter(Boolean))

let timer: ReturnType<typeof setTimeout> | undefined
watch(keyword, (value) => {
  clearTimeout(timer)
  if (!value.trim()) {
    results.value = []
    return
  }
  timer = setTimeout(() => void search(value.trim()), 250)
})

watch(
  selectedIds,
  (ids) => void resolveMissing(ids),
  { immediate: true },
)

/** 回填未缓存的已选用户。 */
async function resolveMissing(ids: string[]) {
  const missing = ids.filter((id) => !directory.value[id])
  if (missing.length === 0) return
  try {
    const users = await $fetch<UserItem[]>('/api/auth/users', {
      params: { ids: missing.join(',') },
      headers: auth.authHeaders(),
    })
    for (const user of users) directory.value[user.id] = user
  } catch {
    // 回填失败不阻断选择
  }
}

/** 关键字搜索。 */
async function search(q: string) {
  loading.value = true
  try {
    const users = await $fetch<UserItem[]>('/api/auth/users', {
      params: { q },
      headers: auth.authHeaders(),
    })
    for (const user of users) directory.value[user.id] = user
    results.value = users.filter(
      (user) => !selectedIds.value.includes(user.id) && !props.excludeIds.includes(user.id),
    )
    activeIndex.value = results.value.length > 0 ? 0 : -1
  } finally {
    loading.value = false
  }
}

/** 键盘导航：上下移动、回车选择、Esc 关闭。 */
function onKeydown(event: KeyboardEvent) {
  if (!results.value.length) return
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    activeIndex.value = (activeIndex.value + 1) % results.value.length
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    activeIndex.value = (activeIndex.value - 1 + results.value.length) % results.value.length
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const user = results.value[activeIndex.value]
    if (user) pick(user)
  } else if (event.key === 'Escape') {
    focused.value = false
  }
}

/** 选中用户。 */
function pick(user: UserItem) {
  directory.value[user.id] = user
  if (props.multiple) {
    emit('update:modelValue', [...selectedIds.value, user.id])
    keyword.value = ''
    results.value = []
  } else {
    emit('update:modelValue', user.id)
    keyword.value = ''
    focused.value = false
  }
}

/** 移除已选。 */
function remove(id: string) {
  if (props.multiple) {
    emit(
      'update:modelValue',
      selectedIds.value.filter((value) => value !== id),
    )
  } else {
    emit('update:modelValue', null)
  }
}
</script>

<template>
  <div class="relative">
    <div
      v-if="multiple && selected.length"
      class="mb-2 flex flex-wrap gap-1.5"
    >
      <span
        v-for="user in selected"
        :key="user.id"
        class="inline-flex items-center gap-1.5 rounded-full bg-primary-50 py-0.5 pr-2 pl-1 text-xs text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
      >
        <UserAvatar :name="user.nickname" :seed="user.id" size="sm" />
        {{ user.nickname }}
        <button class="text-primary-400 hover:text-primary-700" aria-label="移除" @click="remove(user.id)">
          ✕
        </button>
      </span>
    </div>
    <Input
      v-model="keyword"
      :placeholder="placeholder"
      @focusin="focused = true"
      @focusout="setTimeout(() => (focused = false), 150)"
      @keydown="onKeydown"
    />
    <div
      v-if="focused && (results.length || loading || keyword.trim())"
      class="absolute z-20 mt-1 w-full overflow-hidden rounded-[var(--radius-field)] border border-neutral-200 bg-white shadow-[var(--shadow-pop)] dark:border-neutral-700 dark:bg-neutral-900"
    >
      <p v-if="loading" class="px-3 py-2 text-xs text-neutral-400">搜索中…</p>
      <p v-else-if="!results.length" class="px-3 py-2 text-xs text-neutral-400">无匹配用户</p>
      <button
        v-for="(user, index) in results"
        :key="user.id"
        type="button"
        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition"
        :class="
          index === activeIndex
            ? 'bg-primary-50 dark:bg-primary-900/40'
            : 'hover:bg-neutral-50 dark:hover:bg-neutral-800'
        "
        @mousemove="activeIndex = index"
        @mousedown.prevent="pick(user)"
      >
        <UserAvatar :name="user.nickname" :seed="user.id" size="sm" />
        <span class="min-w-0 flex-1">
          <span class="block truncate">{{ user.nickname }}</span>
          <span class="block truncate text-xs text-neutral-400">
            {{ user.department || `@${user.username}` }}
          </span>
        </span>
      </button>
    </div>
    <div
      v-if="!multiple && selected.length"
      class="mt-1.5 flex items-center gap-1.5 text-xs text-neutral-500"
    >
      <UserAvatar :name="selected[0].nickname" :seed="selected[0].id" size="sm" />
      {{ selected[0].nickname }}
      <button class="text-neutral-400 hover:text-danger-500" @click="remove(selected[0].id)">清除</button>
    </div>
  </div>
</template>
