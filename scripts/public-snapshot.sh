#!/usr/bin/env bash
# Build the public repository's first commit from a ref of this one, in a
# new directory. Nothing is pushed: inspect the result, then push it yourself.
#
#   scripts/public-snapshot.sh <ref> <out-dir>
#
# The commit holds the tree of <ref> exactly (`git archive`), with no history,
# authored by CZAR_PUBLIC_AUTHOR (required, e.g. a GitHub noreply
# address), and says how much history it squashes.
set -euo pipefail

ref="${1:?usage: $0 <ref> <out-dir>}"
out="${2:?usage: $0 <ref> <out-dir>}"
author="${CZAR_PUBLIC_AUTHOR:-}"
if [[ -z "$author" ]]; then
  echo "Error: CZAR_PUBLIC_AUTHOR is required to set the public commit author." >&2
  echo "Example: CZAR_PUBLIC_AUTHOR=\"Name <author@users.noreply.github.com>\" $0 <ref> <out-dir>" >&2
  exit 1
fi
root="$(git rev-parse --show-toplevel)"

if [[ -e "$out" ]]; then
  echo "$out already exists; pick a new directory" >&2
  exit 1
fi

commit="$(git -C "$root" rev-parse --verify "$ref^{commit}")"
count="$(git -C "$root" rev-list --count "$commit")"
root_commit="$(git -C "$root" rev-list --max-parents=0 "$commit" | sed -n 1p)"
first="$(git -C "$root" log -1 --format=%as "$root_commit")"
# This repository's own first commit already squashes older history and says
# so ("Squashes the N commits ..., made between <date> and ..."): count those too.
earlier="$(git -C "$root" log -1 --format=%B "$root_commit" | tr '\n' ' ')"
if [[ "$earlier" =~ Squashes\ the\ ([0-9]+)\ commits.*between\ ([0-9-]{10}) ]]; then
  count=$((count - 1 + BASH_REMATCH[1]))
  first="${BASH_REMATCH[2]}"
fi
last="$(git -C "$root" log -1 --format=%as "$commit")"
date="$(git -C "$root" log -1 --format=%aI "$commit")"

mkdir -p "$out"
git -C "$root" archive "$commit" | tar -x -C "$out"
git -C "$out" init -q -b main
git -C "$out" add -A
name="${author% <*}"
email="${author##*<}"
email="${email%>}"
GIT_AUTHOR_NAME="$name" GIT_AUTHOR_EMAIL="$email" GIT_AUTHOR_DATE="$date" \
GIT_COMMITTER_NAME="$name" GIT_COMMITTER_EMAIL="$email" GIT_COMMITTER_DATE="$date" \
  git -C "$out" commit -q -F - <<EOF
Initial commit for public release

Squashes the $count commits of the private repository, made between
$first and $last, into one.
EOF

echo "Built $out: one commit, the tree of $ref ($commit)."
echo "Author: $author"
git -C "$out" log --format='%h %an <%ae> %ad%n%n%B' -1
echo "Check it (cd $out && ./scripts/release-check.py), then push it yourself."
