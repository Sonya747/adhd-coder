import { defineConfig } from "vite";
import { resolve } from "path";

export default defineConfig({
  root: "src",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "src/index.html"),
        onboarding: resolve(__dirname, "src/onboarding.html"),
      },
    },
  },
});
