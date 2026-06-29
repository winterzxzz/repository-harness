const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const electronPath = require("electron");
const { repoRootFromElectronDir, waitForHttp } = require("./backend.cjs");

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

async function main() {
  runChecked("cargo", ["build", "-p", "harness-symphony"]);

  const vite = spawn("npm", ["run", "vite:dev"], {
    cwd: webUiRoot,
    stdio: "inherit",
    shell: process.platform === "win32"
  });

  const cleanup = () => {
    if (!vite.killed) {
      vite.kill();
    }
  };
  process.on("exit", cleanup);
  process.on("SIGINT", () => {
    cleanup();
    process.exit(130);
  });

  await waitForHttp("http://127.0.0.1:5177", { timeoutMs: 30000 });

  const electron = spawn(electronPath, ["electron/main.cjs", "--dev"], {
    cwd: webUiRoot,
    env: {
      ...process.env,
      SYMPHONY_DESKTOP_DEV: "1",
      SYMPHONY_DESKTOP_URL: "http://127.0.0.1:5177"
    },
    stdio: "inherit"
  });

  electron.on("exit", (code) => {
    cleanup();
    process.exit(code ?? 0);
  });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
