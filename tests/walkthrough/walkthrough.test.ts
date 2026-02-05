/**
 * E2E Walkthrough Test for Video Generation
 *
 * Walks through an entire hashcards drill session, capturing numbered
 * screenshots at each stage. Runs in both light and dark color schemes
 * (via Playwright projects). Screenshots go into separate subdirectories
 * and can be assembled into video walkthroughs.
 *
 * The walkthrough server replicates the production HTML structure and
 * uses the production CSS directly, so screenshots reflect the actual
 * look and feel of the built webapp.
 *
 * Prerequisites:
 *   cargo build --manifest-path tests/walkthrough/server/Cargo.toml
 *
 * Usage:
 *   cd tests/walkthrough
 *   npm install
 *   npx playwright test
 *
 * Generate videos from screenshots:
 *   for theme in light dark; do
 *     ffmpeg -framerate 2 -pattern_type glob -i "screenshots/$theme/*.png" \
 *       -c:v libx264 -pix_fmt yuv420p -vf "pad=ceil(iw/2)*2:ceil(ih/2)*2" \
 *       "walkthrough-$theme.mp4"
 *   done
 */

import { test, expect, Page } from "@playwright/test";
import { execSync, ChildProcess, spawn } from "child_process";
import path from "path";
import fs from "fs";

const COLLECTION_DIR = path.join(__dirname, "collection");
const SCREENSHOTS_BASE = path.join(__dirname, "screenshots");
const SERVER_DIR = path.join(__dirname, "server");
const SERVER_BIN = path.join(
  SERVER_DIR,
  "target",
  "debug",
  "walkthrough-server"
);

/** Short pause for visual timing between actions. */
async function pause(ms = 400): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

/** Wait until the server is accepting connections. */
async function waitForServer(url: string, timeoutMs = 30000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Server at ${url} did not start within ${timeoutMs}ms`);
}

test.describe("Hashcards Walkthrough", () => {
  let serverProcess: ChildProcess;
  let port: number;
  let baseUrl: string;
  let counter: number;
  let screenshotsDir: string;

  /** Capture a numbered screenshot. */
  async function capture(page: Page, name: string): Promise<void> {
    counter++;
    const filename = `${String(counter).padStart(3, "0")}-${name}.png`;
    await page.screenshot({
      path: path.join(screenshotsDir, filename),
      fullPage: false,
    });
    console.log(`  📸 ${filename}`);
  }

  test.beforeAll(async ({}, testInfo) => {
    const theme = testInfo.project.name; // "light" or "dark"
    screenshotsDir = path.join(SCREENSHOTS_BASE, theme);
    counter = 0;

    if (fs.existsSync(screenshotsDir)) {
      fs.rmSync(screenshotsDir, { recursive: true });
    }
    fs.mkdirSync(screenshotsDir, { recursive: true });

    // Build the walkthrough server (only once, cargo handles caching)
    console.log(`[${theme}] Building walkthrough server...`);
    execSync("cargo build", { cwd: SERVER_DIR, stdio: "inherit" });

    // Each project gets its own server instance (session state is consumed)
    port = 18900 + Math.floor(Math.random() * 1000);
    baseUrl = `http://127.0.0.1:${port}`;

    console.log(`[${theme}] Starting server on port ${port}...`);
    serverProcess = spawn(SERVER_BIN, [COLLECTION_DIR, String(port)], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    serverProcess.stdout?.on("data", (d) =>
      console.log(`[server] ${d.toString().trim()}`)
    );
    serverProcess.stderr?.on("data", (d) =>
      console.error(`[server] ${d.toString().trim()}`)
    );

    await waitForServer(baseUrl);
    console.log(`[${theme}] Server ready.`);
  });

  test.afterAll(async ({}, testInfo) => {
    const theme = testInfo.project.name;
    if (serverProcess) {
      serverProcess.kill("SIGTERM");
      await new Promise<void>((resolve) => {
        serverProcess.on("close", resolve);
        setTimeout(() => {
          serverProcess.kill("SIGKILL");
          resolve();
        }, 3000);
      });
    }

    console.log(
      `\n✅ [${theme}] Walkthrough complete: ${counter} screenshots`
    );
    console.log(`   Output: ${screenshotsDir}/`);
  });

  test("full drill session walkthrough", async ({ page }, testInfo) => {
    const theme = testInfo.project.name;

    // ================================================================
    // Scene 1: First card — question side
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 1: First Card — Question\n`);

    await page.goto(baseUrl);
    await page.waitForSelector(".card-content");
    await pause(800);
    await capture(page, "first-card-question");

    // ================================================================
    // Scene 2: Reveal the answer
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 2: Reveal Answer\n`);

    await page.click("input#reveal");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "first-card-answer-revealed");

    // ================================================================
    // Scene 3: Rate the card as "Good"
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 3: Rate Card — Good\n`);

    await page.click("input#good");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "second-card-question");

    // ================================================================
    // Scene 4: Use keyboard shortcut — press Space to reveal
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 4: Keyboard Reveal (Space)\n`);

    await page.keyboard.press("Space");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "second-card-answer-revealed");

    // ================================================================
    // Scene 5: Rate as "Hard" using keyboard shortcut
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 5: Rate Card — Hard (keyboard)\n`);

    await page.keyboard.press("2");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "third-card-question");

    // ================================================================
    // Scene 6: Reveal and rate "Easy"
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 6: Rate Card — Easy\n`);

    await page.click("input#reveal");
    await pause(400);
    await capture(page, "third-card-answer-revealed");

    await page.click("input#easy");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "fourth-card-question");

    // ================================================================
    // Scene 7: Undo the last rating
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 7: Undo\n`);

    await page.click("input#undo");
    await page.waitForSelector(".card-content");
    await pause(600);
    await capture(page, "after-undo");

    // ================================================================
    // Scene 8: Continue through remaining cards with varied ratings
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 8: Continue Through Cards\n`);

    let sessionComplete = false;
    let safetyCounter = 0;
    const maxIterations = 30;

    while (!sessionComplete && safetyCounter < maxIterations) {
      safetyCounter++;
      const html = await page.content();

      if (html.includes("Session Completed")) {
        sessionComplete = true;
        break;
      }

      const revealBtn = await page.$("input#reveal");
      if (revealBtn) {
        await page.click("input#reveal");
        await pause(300);

        // Take a screenshot of some revealed cards for variety
        if (safetyCounter % 3 === 0) {
          await capture(page, `card-revealed-${safetyCounter}`);
        }

        // Rate with varied grades
        const grades = [
          "input#good",
          "input#easy",
          "input#good",
          "input#forgot",
        ];
        const gradeBtn = grades[safetyCounter % grades.length];
        const btn = await page.$(gradeBtn);
        if (btn) {
          await page.click(gradeBtn);
        } else {
          await page.click("input#good");
        }
        await pause(300);
      } else {
        const goodBtn = await page.$("input#good");
        if (goodBtn) {
          await page.click("input#good");
          await pause(300);
        } else {
          break;
        }
      }
    }

    // ================================================================
    // Scene 9: Session Completion
    // ================================================================
    console.log(`\n📽️  [${theme}] Scene 9: Session Complete\n`);

    // Make sure we've finished all cards
    if (!sessionComplete) {
      for (let i = 0; i < 30; i++) {
        const h = await page.content();
        if (h.includes("Session Completed")) break;
        const reveal = await page.$("input#reveal");
        if (reveal) {
          await page.click("input#reveal");
          await pause(100);
        }
        const good = await page.$("input#good");
        if (good) {
          await page.click("input#good");
          await pause(100);
        }
      }
    }

    await page.waitForSelector(".finished", { timeout: 10000 });
    await pause(800);
    await capture(page, "session-completed");

    // Verify the completion page
    const completionText = await page.textContent(".finished");
    expect(completionText).toContain("Session Completed");
    expect(completionText).toContain("Cards Reviewed");
    expect(completionText).toContain("Pace");

    // Verify shutdown button is present
    const shutdownBtn = await page.$("input#shutdown");
    expect(shutdownBtn).toBeTruthy();

    console.log(
      `\n🎬 [${theme}] Walkthrough captured ${counter} screenshots.`
    );
  });
});
