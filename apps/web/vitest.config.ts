import { defineConfig } from 'vitest/config'

/**
 * Web 前端单测配置。
 * 覆盖率范围：纯逻辑层（stores / composables / utils）；组件与页面依赖 Nuxt 运行时，
 * 由 E2E 覆盖，暂不计入单测覆盖率。
 */
export default defineConfig({
  test: {
    environment: 'node',
    include: ['stores/**/*.spec.ts', 'composables/**/*.spec.ts', 'utils/**/*.spec.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['stores/**/*.ts', 'composables/**/*.ts', 'utils/**/*.ts'],
      exclude: ['**/*.spec.ts'],
      thresholds: {
        lines: 80,
        statements: 80,
        functions: 80,
        branches: 70,
      },
    },
  },
})
