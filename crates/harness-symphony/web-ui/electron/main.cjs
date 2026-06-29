const path = require("node:path");
const { app, BrowserWindow, dialog, shell } = require("electron");
const {
  developmentBackendBinary,
  findRepoRoot,
  packagedBackendBinary,
  startBackend,
  waitForHttp
} = require("./backend.cjs");

const isDev = process.argv.includes("--dev") || process.env.SYMPHONY_DESKTOP_DEV === "1";
let backend = null;

function desktopPaths() {
  const repoRoot = findRepoRoot({
    electronDir: __dirname,
    resourcesPath: process.resourcesPath,
    cwd: process.cwd()
  });
  if (app.isPackaged && !isDev) {
    return {
      repoRoot,
      binary: packagedBackendBinary(),
      assetDir: path.join(process.resourcesPath, "web-ui-dist"),
      port: 0,
      loadVite: false
    };
  }

  return {
    repoRoot,
    binary: process.env.SYMPHONY_BACKEND_BINARY || developmentBackendBinary(repoRoot),
    assetDir: path.join(repoRoot, "crates", "harness-symphony", "web-ui", "dist"),
    port: Number(process.env.SYMPHONY_BACKEND_PORT || "4317"),
    loadVite: true
  };
}

function createWindow(loadUrl) {
  const window = new BrowserWindow({
    width: 1440,
    height: 940,
    minWidth: 1180,
    minHeight: 760,
    title: "Harness Symphony",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    }
  });

  window.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: "deny" };
  });
  window.loadURL(loadUrl);
  return window;
}

async function start() {
  const paths = desktopPaths();
  backend = startBackend(paths);
  const backendUrl = await backend.urlPromise;
  await waitForHttp(`${backendUrl}/health`, { timeoutMs: 30000 });
  const loadUrl = paths.loadVite
    ? process.env.SYMPHONY_DESKTOP_URL || "http://127.0.0.1:5177"
    : backendUrl;
  createWindow(loadUrl);
}

app.whenReady().then(() => {
  start().catch((error) => {
    dialog.showErrorBox("Harness Symphony failed to start", error.message);
    app.quit();
  });
});

app.on("window-all-closed", () => {
  app.quit();
});

app.on("before-quit", () => {
  if (backend) {
    backend.stop();
  }
});
