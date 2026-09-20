#!/bin/sh

set -eu

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "${script_directory}/.." && pwd)
cd "${repository_root}"

package_id=$(cargo pkgid)
package_version=${package_id##*#}
release_tag="${1:-v${package_version}}"
bundle_name="jev-reranker-${release_tag}-npm-packages.tar.gz"

if [ "${release_tag}" != "v${package_version}" ]; then
  echo "release tag ${release_tag} does not match Cargo version ${package_version}" >&2
  exit 1
fi

# Resolve and cache the exact publishing tool before handling the npm OTP.
npx --yes --registry=https://registry.npmjs.org/ cargo-npm@0.1.4 npm --help >/dev/null

publish_directory=$(mktemp -d)
trap 'rm -rf "${publish_directory}"' EXIT HUP INT TERM

gh release download "${release_tag}" \
  --repo shinpr/jev-reranker \
  --pattern "${bundle_name}" \
  --dir "${publish_directory}"

gh attestation verify "${publish_directory}/${bundle_name}" \
  --repo shinpr/jev-reranker \
  --signer-workflow shinpr/jev-reranker/.github/workflows/release.yml \
  --source-ref "refs/tags/${release_tag}" \
  --deny-self-hosted-runners \
  >/dev/null

mkdir -p "${publish_directory}/extracted"
tar -xzf "${publish_directory}/${bundle_name}" \
  -C "${publish_directory}/extracted"

package_directory="${publish_directory}/extracted/npm"
node scripts/validate-npm-packages.mjs "${package_directory}" "${package_version}"

printf "npm OTP: " >&2
IFS= read -r npm_otp

if [ -z "${npm_otp}" ]; then
  echo "an npm OTP is required" >&2
  exit 1
fi

NPM_CONFIG_OTP="${npm_otp}" npx --offline --yes cargo-npm@0.1.4 npm publish \
  --out-dir "${package_directory}" \
  -- \
  --access public \
  --ignore-scripts \
  --registry=https://registry.npmjs.org/
