<script setup lang="ts">
/**
 * 卡片容器：统一圆角、边框、阴影与内边距。
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
    class="rounded-[var(--radius-card)] border border-neutral-200 bg-white shadow-[var(--shadow-card)] dark:border-neutral-700 dark:bg-neutral-900"
    :class="padded ? 'p-6' : ''"
  >
    <slot />
  </section>
</template>
