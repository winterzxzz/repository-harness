const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const {
  developmentBackendBinary,
  repoRootFromElectronDir,
  requestText,
  startBackend,
  waitForHttp
} = require("./backend.cjs");

const repoRoot = repoRootFromElectronDir();
const webUiRoot = path.resolve(__dirname, "..");
const backendUrl = "http://127.0.0.1:4317";
let backend = null;
let vite = null;

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

async function backendAlreadyRunning() {
  try {
    const response = await requestText(`${backendUrl}/health`);
    return response.statusCode === 200;
  } catch {
    return false;
  }
}

function cleanup() {
  if (vite && !vite.killed) {
    vite.kill();
  }
  if (backend) {
    backend.stop();
  }
}

async function main() {
  if (!(await backendAlreadyRunning())) {
    runChecked("cargo", ["build", "-p", "harness-symphony"]);
    backend = startBackend({
      repoRoot,
      binary: developmentBackendBinary(repoRoot),
      assetDir: path.join(webUiRoot, "dist"),
      port: 4317
    });
    await backend.urlPromise;
    await waitForHttp(`${backendUrl}/health`, { timeoutMs: 30000 });
  }

  vite = spawn("npm", ["run", "vite:dev"], {
    cwd: webUiRoot,
    stdio: "inherit",
    shell: process.platform === "win32"
  });

  vite.on("exit", (code) => {
    cleanup();
    process.exit(code ?? 0);
  });
}

process.on("exit", cleanup);
process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});
process.on("SIGTERM", () => {
  cleanup();
  process.exit(143);
});

main().catch((error) => {
  console.error(error);
  cleanup();
  process.exit(1);
});
