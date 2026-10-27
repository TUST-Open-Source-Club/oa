<script setup lang="ts">
/**
 * 通用按钮：统一风格的基础交互组件。
 * 支持主/次/幽灵/危险四种样式，sm/md/lg 三种尺寸与加载态。
 */
import { computed } from 'vue'

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'
type ButtonSize = 'sm' | 'md' | 'lg'

const props = withDefaults(
  defineProps<{
    /** 视觉样式 */
    variant?: ButtonVariant
    /** 尺寸 */
    size?: ButtonSize
    /** 加载中：禁用交互并显示菊花 */
    loading?: boolean
    /** 是否禁用 */
    disabled?: boolean
    /** 原生 type */
    type?: 'button' | 'submit' | 'reset'
    /** 是否占满父容器宽度 */
    block?: boolean
  }>(),
  {
    variant: 'primary',
    size: 'md',
    loading: false,
    disabled: false,
    type: 'button',
    block: false,
  },
)

const emit = defineEmits<{ (event: 'click', ev: MouseEvent): void }>()

/** 根据 variant/size 组合 class，供测试断言与样式统一。 */
const classes = computed(() => [
  'inline-flex items-center justify-center gap-2 font-medium transition-colors',
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 focus-visible:ring-offset-2',
  'disabled:cursor-not-allowed disabled:opacity-60',
  props.block ? 'w-full' : '',
  {
    sm: 'h-8 px-3 text-sm rounded-[var(--radius-field)]',
    md: 'h-10 px-4 text-sm rounded-[var(--radius-field)]',
    lg: 'h-12 px-6 text-base rounded-[var(--radius-field)]',
  }[props.size],
  {
    primary: 'bg-primary-600 text-white hover:bg-primary-700 active:bg-primary-800',
    secondary:
      'border border-neutral-300 bg-white text-neutral-800 hover:bg-neutral-50 dark:border-neutral-600 dark:bg-neutral-800 dark:text-neutral-100 dark:hover:bg-neutral-700',
    ghost: 'text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800',
    danger: 'bg-danger-500 text-white hover:bg-danger-700',
  }[props.variant],
])

/** 仅在可用状态下 emit click，避免 loading/disabled 误触。 */
function onClick(event: MouseEvent) {
  if (props.loading || props.disabled) return
  emit('click', event)
}
</script>

<template>
  <button :type="type" :class="classes" :disabled="disabled || loading" @click="onClick">
    <span
      v-if="loading"
      class="size-4 animate-spin rounded-full border-2 border-current border-t-transparent"
      aria-hidden="true"
    />
    <slot />
  </button>
</template>
