import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// `npm run dev` proxies the API to a local server (MONICA_API, default http://127.0.0.1:8000).
const api = process.env.MONICA_API ?? "http://127.0.0.1:8000";

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith("m3e-")
        }
      }
    })
  ],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: api, changeOrigin: false },
      "/alive": { target: api }
    }
  },
  build: {
    chunkSizeWarningLimit: 1500
  }
});
