#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "${repository_root}"

default_branch=${DEFAULT_BRANCH:-main}
release_workflow=${RELEASE_WORKFLOW:-release.yml}
dry_run=false
resume=false

usage() {
    echo "Usage: scripts/release.sh [--dry-run] [--resume] [vX.Y.Z[-PRERELEASE]]" >&2
}

while (($# > 0)); do
    case "$1" in
        --dry-run)
            dry_run=true
            shift
            ;;
        --resume)
            resume=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --*)
            usage
            exit 2
            ;;
        *)
            if [[ -n "${release_tag:-}" ]]; then
                usage
                exit 2
            fi
            release_tag=$1
            shift
            ;;
    esac
done

for command in git gh python3; do
    command -v "${command}" >/dev/null 2>&1 || {
        echo "Release stopped: ${command} is required." >&2
        exit 1
    }
done

current_branch=$(git branch --show-current)
if [[ "${current_branch}" != "${default_branch}" ]]; then
    echo "Release stopped: run from ${default_branch}; current branch is ${current_branch}." >&2
    exit 1
fi

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "Release stopped: the working tree is not clean." >&2
    git status --short >&2
    exit 1
fi

git fetch --quiet origin "${default_branch}" --tags
source_sha=$(git rev-parse HEAD)
remote_sha=$(git rev-parse "origin/${default_branch}")
if [[ "${source_sha}" != "${remote_sha}" ]]; then
    echo "Release stopped: local ${default_branch} differs from origin/${default_branch}." >&2
    exit 1
fi

version=$(scripts/read-version.sh)
if [[ ! "${version}" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "Release stopped: invalid SemVer product version ${version}." >&2
    exit 1
fi

release_tag=${release_tag:-"v${version}"}
if [[ "${release_tag}" != "v${version}" ]]; then
    echo "Release stopped: tag ${release_tag} does not match product version ${version}." >&2
    exit 1
fi

python3 - "${version}" <<'PY'
import re
import sys
from pathlib import Path

version = sys.argv[1]
text = Path("CHANGELOG.md").read_text(encoding="utf-8")
if not re.search(rf"^## \[{re.escape(version)}\](?:\s+-\s+\d{{4}}-\d{{2}}-\d{{2}})?\s*$", text, re.MULTILINE):
    raise SystemExit(f"Release stopped: CHANGELOG.md has no section for {version}.")
PY

gh auth status >/dev/null
tag_exists=false
release_state=""
if git ls-remote --exit-code --tags origin "refs/tags/${release_tag}" >/dev/null 2>&1; then
    tag_exists=true
fi
if gh release view "${release_tag}" --json isDraft,isPrerelease >/dev/null 2>&1; then
    release_state=$(gh release view "${release_tag}" --json isDraft,isPrerelease --jq 'if .isDraft then "draft" else "published" end')
fi

if [[ "${resume}" == false && ("${tag_exists}" == true || -n "${release_state}") ]]; then
    echo "Release stopped: ${release_tag} already has a remote tag or Release; use --resume only for a matching interrupted draft." >&2
    exit 1
fi
if [[ "${resume}" == true && "${release_state}" == "published" ]]; then
    echo "Release stopped: ${release_tag} is already public and cannot be resumed." >&2
    exit 1
fi

echo "Release tag: ${release_tag}"
echo "Source SHA: ${source_sha}"
echo "Mode: $([[ "${resume}" == true ]] && echo resume || echo new)"
if [[ "${dry_run}" == true ]]; then
    echo "Dry run passed; no workflow was dispatched."
    exit 0
fi

gh workflow run "${release_workflow}" \
    --ref "${default_branch}" \
    -f "release_tag=${release_tag}" \
  -f "expected_source_sha=${source_sha}" \
  -f "resume=${resume}" \
  -f "publish_oss=false" \
  -f "release_sequence=0"

repository=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
echo "Release ${release_tag} was handed to GitHub Actions."
echo "The local terminal can now be closed."
echo "Progress: https://github.com/${repository}/actions"
