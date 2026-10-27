<script setup lang="ts">
/** 通用弹窗：遮罩 + 卡片 + 头/脚插槽，Esc 与点击遮罩关闭。 */
const props = defineProps<{ open: boolean; title?: string; width?: string }>()
const emit = defineEmits<{ close: [] }>()

/** Esc 关闭。 */
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') emit('close')
}

watch(
  () => props.open,
  (value) => {
    if (typeof document === 'undefined') return
    document.body.style.overflow = value ? 'hidden' : ''
  },
)

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => {
  window.removeEventListener('keydown', onKey)
  if (typeof document !== 'undefined') document.body.style.overflow = ''
})
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div class="absolute inset-0 bg-neutral-900/40 backdrop-blur-sm" @click="emit('close')" />
      <div
        class="relative w-full rounded-[var(--radius-card)] bg-white shadow-[var(--shadow-pop)] dark:bg-neutral-900"
        :class="width ?? 'max-w-md'"
      >
        <header
          class="flex items-center justify-between border-b border-neutral-200 px-5 py-3 dark:border-neutral-800"
        >
          <h2 class="text-sm font-semibold">{{ title }}</h2>
          <button
            class="rounded p-1 text-neutral-400 transition hover:bg-neutral-100 hover:text-neutral-600 dark:hover:bg-neutral-800"
            aria-label="关闭"
            @click="emit('close')"
          >
            ✕
          </button>
        </header>
        <div class="max-h-[70vh] overflow-y-auto px-5 py-4">
          <slot />
        </div>
        <footer
          v-if="$slots.footer"
          class="flex justify-end gap-2 border-t border-neutral-200 px-5 py-3 dark:border-neutral-800"
        >
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>
