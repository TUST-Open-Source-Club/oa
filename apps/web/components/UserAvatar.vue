<script setup lang="ts">
/** 用户头像：昵称首字 + 按 ID/名称哈希的底色（无头像上传时统一风格）。 */
const props = withDefaults(
  defineProps<{ name?: string | null; seed?: string | null; size?: 'sm' | 'md' | 'lg' }>(),
  { name: '', seed: '', size: 'md' },
)

const palette = [
  'bg-primary-500',
  'bg-emerald-400',
  'bg-amber-400',
  'bg-rose-400',
  'bg-sky-400',
  'bg-violet-400',
]

const initial = computed(() => (props.name?.trim()?.[0] ?? '?').toUpperCase())
const color = computed(() => {
  const key = props.seed || props.name || '?'
  let hash = 0
  for (const char of key) hash = (hash * 31 + char.charCodeAt(0)) >>> 0
  return palette[hash % palette.length]
})
const sizeClass = computed(
  () => ({ sm: 'size-6 text-[10px]', md: 'size-8 text-xs', lg: 'size-10 text-sm' })[props.size],
)
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center justify-center rounded-full font-medium text-white select-none"
    :class="[color, sizeClass]"
  >
    {{ initial }}
  </span>
</template>
