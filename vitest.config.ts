import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// 覆盖率约束真实业务和交互模块；测试本身、类型声明和入口装配不参与分母。
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true,
    clearMocks: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary", "lcov"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/**/*.d.ts", "src/test/**", "src/main.tsx"],
      thresholds: { lines: 80, statements: 80, functions: 80, branches: 80 },
    },
  },
});
