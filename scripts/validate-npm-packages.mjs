#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const [packageRootArgument, expectedVersion] = process.argv.slice(2);

if (!packageRootArgument || !expectedVersion) {
  fail("usage: validate-npm-packages.mjs <package-directory> <version>");
}

const packageRoot = path.resolve(packageRootArgument);
const platformPackages = [
  { suffix: "darwin-arm64", os: "darwin", cpu: "arm64", binary: "jev-reranker" },
  { suffix: "darwin-x64", os: "darwin", cpu: "x64", binary: "jev-reranker" },
  { suffix: "linux-arm64", os: "linux", cpu: "arm64", binary: "jev-reranker" },
  { suffix: "linux-x64", os: "linux", cpu: "x64", binary: "jev-reranker" },
  { suffix: "win32-arm64", os: "win32", cpu: "arm64", binary: "jev-reranker.exe" },
  { suffix: "win32-x64", os: "win32", cpu: "x64", binary: "jev-reranker.exe" },
];
const optionalDependencies = Object.fromEntries(
  platformPackages.map(({ suffix }) => [`@shinpr/jev-reranker-${suffix}`, expectedVersion]),
);

validatePackage({
  directory: path.join(packageRoot, "jev-reranker"),
  expectedName: "jev-reranker",
  expectedFiles: ["LICENSE", "README.md", "bin/jev-reranker.js", "package.json"],
  validateMetadata(packageJson) {
    assertExactObject(packageJson.bin, { "jev-reranker": "bin/jev-reranker.js" }, "jev-reranker.bin");
    assertExactObject(
      packageJson.optionalDependencies,
      optionalDependencies,
      "jev-reranker.optionalDependencies",
    );
  },
});

for (const { suffix, os, cpu, binary } of platformPackages) {
  const packageName = `@shinpr/jev-reranker-${suffix}`;
  validatePackage({
    directory: path.join(packageRoot, "@shinpr", `jev-reranker-${suffix}`),
    expectedName: packageName,
    expectedFiles: ["LICENSE", binary, "package.json"],
    validateMetadata(packageJson) {
      assertExactArray(packageJson.os, [os], `${packageName}.os`);
      assertExactArray(packageJson.cpu, [cpu], `${packageName}.cpu`);
      if (Object.hasOwn(packageJson, "bin")) {
        fail(`${packageName} must not define bin`);
      }
      if (Object.hasOwn(packageJson, "optionalDependencies")) {
        fail(`${packageName} must not define optionalDependencies`);
      }
    },
  });
}

function validatePackage({ directory, expectedName, expectedFiles, validateMetadata }) {
  const packageJsonPath = path.join(directory, "package.json");
  let packageJson;

  try {
    packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  } catch (error) {
    fail(`cannot read ${packageJsonPath}: ${error.message}`);
  }

  if (packageJson.name !== expectedName) {
    fail(`${expectedName} has unexpected name: ${String(packageJson.name)}`);
  }
  if (packageJson.version !== expectedVersion) {
    fail(`${expectedName} has unexpected version: ${String(packageJson.version)}`);
  }

  for (const field of [
    "scripts",
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "bundleDependencies",
    "bundledDependencies",
  ]) {
    if (Object.hasOwn(packageJson, field)) {
      fail(`${expectedName} must not define ${field}`);
    }
  }

  assertExactObject(packageJson.publishConfig, { access: "public" }, `${expectedName}.publishConfig`);
  validateMetadata(packageJson);

  const actualFiles = listFiles(directory).sort();
  const wantedFiles = [...expectedFiles].sort();
  if (JSON.stringify(actualFiles) !== JSON.stringify(wantedFiles)) {
    fail(
      `${expectedName} contains unexpected files: expected ${wantedFiles.join(", ")}; got ${actualFiles.join(", ")}`,
    );
  }
}

function listFiles(directory, relativeDirectory = "") {
  let entries;
  try {
    entries = fs.readdirSync(path.join(directory, relativeDirectory), { withFileTypes: true });
  } catch (error) {
    fail(`cannot read package directory ${directory}: ${error.message}`);
  }

  const files = [];
  for (const entry of entries) {
    const relativePath = path.posix.join(relativeDirectory, entry.name);
    const absolutePath = path.join(directory, relativePath);
    const stats = fs.lstatSync(absolutePath);

    if (stats.isSymbolicLink()) {
      fail(`package contains a symbolic link: ${relativePath}`);
    }
    if (stats.isDirectory()) {
      files.push(...listFiles(directory, relativePath));
    } else if (stats.isFile()) {
      files.push(relativePath);
    } else {
      fail(`package contains an unsupported file type: ${relativePath}`);
    }
  }
  return files;
}

function assertExactObject(actual, expected, label) {
  if (
    !actual ||
    Array.isArray(actual) ||
    typeof actual !== "object" ||
    JSON.stringify(sortObject(actual)) !== JSON.stringify(sortObject(expected))
  ) {
    fail(`${label} does not match the expected value`);
  }
}

function assertExactArray(actual, expected, label) {
  if (!Array.isArray(actual) || JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} does not match the expected value`);
  }
}

function sortObject(value) {
  return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)));
}

function fail(message) {
  console.error(`npm package validation failed: ${message}`);
  process.exit(1);
}
