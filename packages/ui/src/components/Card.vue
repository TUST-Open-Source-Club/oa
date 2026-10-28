<script setup lang="ts">
/**
 * 卡片容器：统一圆角、边框、表面色与内边距。
 *
 * 层级靠边框 + 表面色表达，阴影只保留极浅一层。
 */
withDefaults(
  defineProps<{
    /** 是否使用内边距 */
    padded?: boolean
  }>(),
  { padded: true },
)
</script>

<template>
  <section
    class="rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    :class="padded ? 'p-5' : ''"
  >
    <slot />
  </section>
</template>
