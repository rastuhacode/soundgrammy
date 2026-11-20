#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

target=$1
output=$2
source="dist/$target/release"
version=$(tr -d '\r\n' < version.txt)
mkdir -p "$output"

stage() {
  local name=$1
  shift
  if [[ $# != 1 || ! -s ${1:-} ]]; then
    echo "Expected one nonempty $name in $source, found $#" >&2
    exit 1
  fi
  cp "$1" "$output/$name"
}

case "$target" in
  aarch64-apple-darwin|x86_64-apple-darwin)
    arch=${target%%-*}
    if [[ $arch == aarch64 ]]; then mac_arch=arm64; else mac_arch=x86_64; fi
    app="$source/SoundGrammy.app"
    codesign --verify --deep --strict "$app"
    lipo "$app/Contents/MacOS/soundgrammy" -verify_arch "$mac_arch"
    dmg="SoundGrammy_${version}_${arch}.dmg"
    stage "$dmg" "$source"/*.dmg
    hdiutil verify "$output/$dmg"
    ;;
  x86_64-pc-windows-msvc)
    stage "SoundGrammy_${version}_x64-setup.exe" "$source"/*.exe
    stage "SoundGrammy_${version}_x64.msi" "$source"/*.msi
    ;;
  *)
    echo "Unsupported desktop release target: $target" >&2
    exit 1
    ;;
esac
