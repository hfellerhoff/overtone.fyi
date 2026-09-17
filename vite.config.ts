import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    // The AudioWorklet module must be a real file: some browsers refuse
    // data: URLs in audioWorklet.addModule.
    assetsInlineLimit: (filePath) =>
      filePath.endsWith("capture-worklet.js") ? false : undefined,
  },
  server: {
    port: 3000,
    strictPort: true,
  },
  preview: {
    port: 3000,
    strictPort: true,
  },
  // Tauri expects a fixed port and no clearing of the terminal.
  clearScreen: false,
});
