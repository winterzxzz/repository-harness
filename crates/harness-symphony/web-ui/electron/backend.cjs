const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { spawn } = require("node:child_process");

function repoRootFromElectronDir() {
  return path.resolve(__dirname, "../../../..");
}

function looksLikeRepoRoot(candidate) {
  return (
    candidate &&
    fs.existsSync(path.join(candidate, "harness.db")) &&
    fs.existsSync(path.join(candidate, "crates", "harness-symphony"))
  );
}

function ancestors(startPath) {
  const result = [];
  let current = path.resolve(startPath);
  for (;;) {
    result.push(current);
    const parent = path.dirname(current);
    if (parent === current) {
      return result;
    }
    current = parent;
  }
}

function findRepoRoot(options = {}) {
  const explicit = options.envRepoRoot || process.env.SYMPHONY_REPO_ROOT;
  if (explicit) {
    if (!looksLikeRepoRoot(explicit)) {
      throw new Error(`SYMPHONY_REPO_ROOT does not look like a Symphony repo: ${explicit}`);
    }
    return path.resolve(explicit);
  }

  const starts = [
    options.cwd || process.cwd(),
    options.electronDir || __dirname,
    options.resourcesPath || process.resourcesPath
  ].filter(Boolean);

  for (const start of starts) {
    for (const candidate of ancestors(start)) {
      if (looksLikeRepoRoot(candidate)) {
        return candidate;
      }
    }
  }

  throw new Error(
    "Could not find a Symphony repo root. Launch the app from the repository or set SYMPHONY_REPO_ROOT."
  );
}

function platformBinaryName() {
  return process.platform === "win32" ? "harness-symphony.exe" : "harness-symphony";
}

function developmentBackendBinary(repoRoot) {
  return path.join(repoRoot, "target", "debug", platformBinaryName());
}

function packagedBackendBinary(resourcesPath = process.resourcesPath) {
  return path.join(resourcesPath, "bin", platformBinaryName());
}

function assertExecutable(binaryPath) {
  if (!fs.existsSync(binaryPath)) {
    throw new Error(`Harness Symphony backend binary is missing: ${binaryPath}`);
  }
}

function requestText(url) {
  return new Promise((resolve, reject) => {
    const request = http.get(url, (response) => {
      let body = "";
      response.setEncoding("utf8");
      response.on("data", (chunk) => {
        body += chunk;
      });
      response.on("end", () => {
        resolve({
          statusCode: response.statusCode ?? 0,
          body
        });
      });
    });
    request.on("error", reject);
    request.setTimeout(1500, () => {
      request.destroy(new Error(`Timed out waiting for ${url}`));
    });
  });
}

async function waitForHttp(url, options = {}) {
  const timeoutMs = options.timeoutMs ?? 30000;
  const startedAt = Date.now();
  let lastError;

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const response = await requestText(url);
      if (response.statusCode >= 200 && response.statusCode < 500) {
        return response;
      }
      lastError = new Error(`${url} returned HTTP ${response.statusCode}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  throw lastError ?? new Error(`Timed out waiting for ${url}`);
}

function startBackend(options) {
  const repoRoot = options.repoRoot;
  const binary = options.binary;
  const assetDir = options.assetDir;
  const host = options.host ?? "127.0.0.1";
  const port = options.port ?? 0;
  assertExecutable(binary);

  const child = spawn(
    binary,
    ["--repo-root", repoRoot, "web", "--host", host, "--port", String(port)],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        HARNESS_SYMPHONY_WEB_DIST_DIR: assetDir
      },
      stdio: ["ignore", "pipe", "pipe"]
    }
  );

  let settled = false;
  let stdout = "";
  let stderr = "";

  const urlPromise = new Promise((resolve, reject) => {
    const fail = (error) => {
      if (!settled) {
        settled = true;
        reject(error);
      }
    };

    const parseUrl = (chunk) => {
      const text = chunk.toString();
      stdout += text;
      const match = text.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match && !settled) {
        settled = true;
        resolve(match[0]);
      }
    };

    child.stdout.on("data", parseUrl);
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", fail);
    child.on("exit", (code, signal) => {
      if (!settled) {
        fail(
          new Error(
            `Harness Symphony backend exited before startup (code ${code}, signal ${signal}). ${stderr || stdout}`
          )
        );
      }
    });
  });

  return {
    process: child,
    urlPromise,
    async healthUrl() {
      const url = await urlPromise;
      return `${url}/health`;
    },
    stop() {
      if (!child.killed) {
        child.kill();
      }
    }
  };
}

module.exports = {
  assertExecutable,
  developmentBackendBinary,
  findRepoRoot,
  packagedBackendBinary,
  repoRootFromElectronDir,
  requestText,
  startBackend,
  waitForHttp
};
