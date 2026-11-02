import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    solid(),
    tailwindcss(),
    VitePWA({
      // The page registers the worker itself, so the Tauri shell never installs
      // one; the plugin only generates `sw.js` and the manifest.
      injectRegister: false,
      registerType: "autoUpdate",
      manifest: {
        name: "Daywerk",
        short_name: "Daywerk",
        description: "Zeiterfassung und Aufgaben – HGS Tschernischow",
        lang: "de",
        start_url: "/",
        scope: "/",
        display: "standalone",
        background_color: "#f4f8f4",
        theme_color: "#2a7647",
        categories: ["business", "productivity"],
        icons: [
          { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
          { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "/icon-maskable-512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
      workbox: {
        // Only the app shell is cached. API responses (bookings, tasks,
        // sessions) must never be served from the cache.
        globPatterns: ["**/*.{js,css,html,png,svg,ico,webmanifest,woff2}"],
        navigateFallback: "/index.html",
        navigateFallbackDenylist: [/^\/api\//],
        cleanupOutdatedCaches: true,
        clientsClaim: true,
        skipWaiting: true,
      },
      devOptions: { enabled: false },
    }),
  ],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:9095",
    },
  },
  build: {
    target: "es2022",
  },
});
