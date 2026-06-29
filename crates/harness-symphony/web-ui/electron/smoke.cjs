const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const electronPath = require("electron");
const {
  developmentBackendBinary,
  findRepoRoot,
  repoRootFromElectronDir,
  requestText,
  startBackend,
  waitForHttp
} = require("./backend.cjs");

const repoRoot = repoRootFromElectronDir();
const webUiRoot = path.resolve(__dirname, "..");

function runChecked(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    stdio: "inherit",
    shell: process.platform === "win32"
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function checkSyntax(file) {
  runChecked(process.execPath, ["--check", file], { cwd: webUiRoot });
}

async function main() {
  checkSyntax(path.join(webUiRoot, "electron", "backend.cjs"));
  checkSyntax(path.join(webUiRoot, "electron", "main.cjs"));
  checkSyntax(path.join(webUiRoot, "electron", "dev.cjs"));
  if (!fs.existsSync(electronPath)) {
    throw new Error(`Electron executable is missing: ${electronPath}`);
  }
  const discoveredRoot = findRepoRoot({
    cwd: path.join(repoRoot, "crates", "harness-symphony", "web-ui", "desktop-dist", "mac-arm64"),
    electronDir: __dirname
  });
  if (discoveredRoot !== repoRoot) {
    throw new Error(`Repo root discovery returned ${discoveredRoot}, expected ${repoRoot}`);
  }

  runChecked("cargo", ["build", "-p", "harness-symphony"]);

  const backend = startBackend({
    repoRoot,
    binary: developmentBackendBinary(repoRoot),
    assetDir: path.join(webUiRoot, "dist"),
    port: 0
  });

  try {
    const baseUrl = await backend.urlPromise;
    await waitForHttp(`${baseUrl}/health`, { timeoutMs: 30000 });
    const root = await waitForHttp(baseUrl, { timeoutMs: 30000 });
    if (!root.body.includes("<div id=\"root\"></div>")) {
      throw new Error("Desktop backend did not serve the built React index");
    }

    const board = await requestText(`${baseUrl}/api/board`);
    if (board.statusCode !== 200) {
      throw new Error(`/api/board returned HTTP ${board.statusCode}`);
    }
    const parsed = JSON.parse(board.body);
    if (!Array.isArray(parsed.items)) {
      throw new Error("/api/board did not return an items array");
    }
    console.log(`Desktop smoke passed at ${baseUrl}`);
  } finally {
    backend.stop();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
