import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "walkthrough.test.ts",
  timeout: 120_000,
  fullyParallel: false,
  workers: 1,
  use: {
    viewport: { width: 1280, height: 800 },
    screenshot: "off",
    launchOptions: {
      executablePath:
        process.env.CHROMIUM_PATH ||
        "/root/.cache/ms-playwright/chromium-1194/chrome-linux/chrome",
    },
  },
  projects: [
    {
      name: "light",
      use: { browserName: "chromium", colorScheme: "light" },
    },
    {
      name: "dark",
      use: { browserName: "chromium", colorScheme: "dark" },
    },
  ],
});
