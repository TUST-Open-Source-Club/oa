<script setup lang="ts">
/**
 * 通用按钮：统一风格的基础交互组件。
 *
 * 风格：工具型产品的紧凑按钮（28/32/40px 三档），主按钮用强调色 + 顶部内高光，
 * 次要按钮靠边框区分层级，幽灵按钮只在 hover 时出现底色。
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
  'inline-flex select-none items-center justify-center gap-1.5 rounded-[var(--radius-field)] font-medium whitespace-nowrap',
  'transition-[background-color,border-color,color,box-shadow] duration-150',
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500/45 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--canvas)]',
  'disabled:cursor-not-allowed disabled:opacity-55',
  props.block ? 'w-full' : '',
  {
    sm: 'h-7 px-2.5 text-[12px]',
    md: 'h-8 px-3 text-[13px]',
    lg: 'h-10 px-4 text-sm',
  }[props.size],
  {
    primary:
      'bg-primary-600 text-white shadow-[var(--shadow-inset)] ring-1 ring-primary-700/30 ring-inset hover:bg-primary-700 active:bg-primary-800',
    secondary:
      'border border-[var(--border-strong)] bg-[var(--surface)] text-neutral-800 hover:bg-[var(--surface-hover)] active:bg-[var(--surface-muted)] dark:text-neutral-100',
    ghost: 'text-neutral-600 hover:bg-[var(--surface-hover)] dark:text-neutral-300',
    danger: 'bg-danger-500 text-white shadow-[var(--shadow-inset)] hover:bg-danger-700',
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
      class="size-3.5 animate-spin rounded-full border-2 border-current border-t-transparent"
      aria-hidden="true"
    />
    <slot />
  </button>
</template>
