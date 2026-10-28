<template>
  <!-- 登录/激活等未登录页面：居中卡片 + 顶部极淡的品牌光晕 -->
  <main class="relative flex min-h-screen items-center justify-center overflow-hidden bg-[var(--canvas)] px-4 py-10">
    <div
      aria-hidden="true"
      class="pointer-events-none absolute inset-x-0 top-0 h-[420px] bg-[radial-gradient(60%_100%_at_50%_0%,color-mix(in_oklab,var(--color-primary-500)_14%,transparent),transparent)]"
    />
    <div class="relative w-full max-w-[380px]">
      <div class="mb-6 flex items-center justify-center gap-2">
        <span
          class="flex size-7 items-center justify-center rounded-md bg-primary-600 text-[12px] font-semibold text-white"
        >
          社
        </span>
        <span class="text-[15px] font-semibold tracking-[-0.01em] text-[var(--text-strong)]">
          {{ $config.public.appName }}
        </span>
      </div>
      <slot />
      <p class="mt-6 text-center text-[11px] text-neutral-400">
        账号由管理员创建，遇到问题请联系社团管理员
      </p>
    </div>
  </main>
</template>
