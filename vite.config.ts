import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// 桌面构建与开发服务配置分离；执行build不会启动本地服务。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { strictPort: true, port: 1420 },
  build: { target: "es2022", sourcemap: true },
});
