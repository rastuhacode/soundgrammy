#!/usr/bin/env bash
set -euo pipefail

# Check the remote tag again after the builds. An annotated tag has its own
# object ID, so use the peeled commit when present.
tag_ref="refs/tags/$RELEASE_TAG"
refs=$(git ls-remote --exit-code origin "$tag_ref" "$tag_ref^{}")
tag_commit=$(awk -v ref="$tag_ref" '
  $2 == ref { tag = $1 }
  $2 == ref "^{}" { peeled = $1 }
  END { print (peeled != "" ? peeled : tag) }
' <<< "$refs")
if [[ $tag_commit != "$RELEASE_COMMIT" ]]; then
  echo "Release tag $RELEASE_TAG moved during the build" >&2
  exit 1
fi

version=${RELEASE_TAG#v}
assets=(
  "SoundGrammy_${version}_aarch64.dmg"
  "SoundGrammy_${version}_x86_64.dmg"
  "SoundGrammy_${version}_x64-setup.exe"
  "SoundGrammy_${version}_x64.msi"
  "SoundGrammy_${version}_aarch64.apk"
)
for asset in "${assets[@]}"; do
  test -s "$1/$asset" || { echo "Missing release asset: $asset" >&2; exit 1; }
done

matching_releases() {
  gh api --paginate --slurp "repos/$GITHUB_REPOSITORY/releases?per_page=100" |
    jq --arg tag "$RELEASE_TAG" '[.[][] | select(.tag_name == $tag)]'
}

releases=$(matching_releases)
if [[ $(jq 'length' <<< "$releases") == 0 ]]; then
  gh release create "$RELEASE_TAG" --verify-tag --draft --title "SoundGrammy $RELEASE_TAG"
  releases=$(matching_releases)
fi
if ! jq -e 'length == 1 and .[0].draft == true' <<< "$releases" >/dev/null; then
  echo "Expected exactly one draft release for $RELEASE_TAG" >&2
  exit 1
fi

# A failed upload can be retried with the same artifacts. Never delete or
# replace an existing asset as part of an automated run.
upload=()
for asset in "${assets[@]}"; do
  matches=$(jq --arg name "$asset" '[.[0].assets[] | select(.name == $name)]' <<< "$releases")
  if [[ $(jq 'length' <<< "$matches") == 0 ]]; then
    upload+=("$1/$asset")
    continue
  fi
  digest="sha256:$(sha256sum "$1/$asset" | cut -d' ' -f1)"
  if ! jq -e --arg digest "$digest" 'length == 1 and .[0].digest == $digest' <<< "$matches" >/dev/null; then
    echo "Existing asset differs or has no matching digest: $asset" >&2
    exit 1
  fi
done
if (( ${#upload[@]} > 0 )); then
  gh release upload "$RELEASE_TAG" "${upload[@]}"
fi
