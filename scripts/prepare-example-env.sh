#!/usr/bin/env bash
set -euo pipefail

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo "This script is meant to be sourced, not executed directly." >&2
  echo "Usage: source scripts/prepare-example-env.sh <example_dir> <project_root>" >&2
  exit 1
fi

example_dir="${1:?example_dir is required}"
project_root="${2:?project_root is required}"

pyproject_dir=""
if [[ -f "$example_dir/pyproject.toml" ]]; then
  pyproject_dir="$example_dir"
else
  shopt -s nullglob
  pyprojects=("$example_dir"/*/pyproject.toml)
  shopt -u nullglob
  if [[ "${#pyprojects[@]}" -eq 1 ]]; then
    pyproject_dir="$(dirname "${pyprojects[0]}")"
  fi
fi

if [[ -n "$pyproject_dir" ]]; then
  # shellcheck disable=SC1091
  source "$project_root/scripts/deps-common.sh"
  require_uv

  venv_dir="$pyproject_dir/.venv"
  PROJECT_ROOT="$project_root" \
    UV_PROJECT_ENVIRONMENT="$venv_dir" \
    uv sync --project "$pyproject_dir" --frozen
  export VIRTUAL_ENV="$venv_dir"
  export PATH="$VIRTUAL_ENV/bin:$PATH"
fi

if [[ -d "$example_dir/bin" ]]; then
  export PATH="$example_dir/bin:$PATH"
fi

export PYTHONPATH="$example_dir${PYTHONPATH:+:$PYTHONPATH}"
