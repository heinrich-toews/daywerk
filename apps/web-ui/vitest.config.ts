import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // The clock helpers are pure; no DOM is needed.
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
