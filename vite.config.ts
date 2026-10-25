import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri 开发服务器固定端口（GitTree 用 1420，这里错开）
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1421, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/target/**"] } },
  build: { target: "es2022" },
});
