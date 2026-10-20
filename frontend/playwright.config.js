import { defineConfig, devices } from "@playwright/test";

const API = process.env.E2E_API ?? "http://localhost:4444";
const PORT = 5199;

export default defineConfig({
  testDir: "e2e",
  globalSetup: "./e2e/setup.js",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    storageState: "e2e/.auth.json",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    { name: "android", use: { ...devices["Pixel 7"] } },
    { name: "iphone", use: { ...devices["iPhone 13"] } },
    { name: "iphone-se", use: { ...devices["iPhone SE"] } },
    { name: "iphone-max", use: { ...devices["iPhone 14 Pro Max"] } },
    { name: "ipad-mini", use: { ...devices["iPad Mini"] } },
    { name: "ipad-pro-landscape", use: { ...devices["iPad Pro 11 landscape"] } },
    { name: "android-tablet", use: { ...devices["Galaxy Tab S4"] } },
  ],
  webServer: [
    {
      command: "docker compose up db api",
      cwd: "..",
      url: `${API}/health`,
      reuseExistingServer: true,
      timeout: 15 * 60 * 1000,
    },
    {
      command: `vite --port ${PORT} --strictPort`,
      env: { VITE_API_TARGET: API },
      url: `http://localhost:${PORT}`,
      reuseExistingServer: true,
    },
  ],
});
