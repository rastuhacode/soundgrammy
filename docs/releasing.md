# Releasing SoundGrammy

SoundGrammy uses [Release Please](https://github.com/googleapis/release-please) to turn Conventional Commits on `main` into a versioned release pull request, changelog entries, a Git tag, and a draft GitHub Release. The Tauri build workflow then uploads macOS, Windows, and Android arm64 artifacts to that draft.

## Workflow

1. Merge changes into `main`, preferably with squash merge and a Conventional Commit title.
2. The **Release Please** workflow opens or refreshes a release pull request. It proposes the next version, updates `CHANGELOG.md`, and synchronizes `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `.release-please-manifest.json`.
3. Continue merging ordinary changes. Release Please keeps the same release pull request current.
4. Review the generated changelog and version changes, then merge the release pull request when the release is ready.
5. Release Please creates the `vX.Y.Z` tag and a draft GitHub Release. In the same workflow run, the reusable **Build release** workflow checks out the exact release commit, verifies every version, builds Intel and Apple Silicon macOS bundles plus Windows bundles and a signed Android arm64 APK, and uploads them to the draft.
6. Test the uploaded artifacts and edit the release notes if needed. Publish the draft GitHub Release when it is ready for users.

The release is deliberately left as a draft so incomplete, unsigned, or untested binaries are never published automatically.

The Android APK targets arm64 devices. Android device acceptance remains outstanding, so the draft APK is a preview artifact rather than an officially supported platform release.

## Commit titles and version bumps

Release Please derives SemVer changes from commit messages:

| Commit title | Result |
| --- | --- |
| `fix: prevent stalled playback` | Patch release |
| `feat: add queue export` | Minor release |
| `feat!: replace playlist recipe format` | Breaking release; while below 1.0, this is configured as a minor bump |
| `docs: clarify setup` or `chore: update tooling` | May appear in notes but does not trigger a version bump by itself |

Use a `Release-As: 1.0.0` footer on a commit only when an explicit version override is required. Prefer squash merging pull requests so the final pull request title becomes the single release-note-worthy commit on `main`.

To override the next version without adding a commit, manually run **Release Please** and enter a SemVer value such as `1.0.0` in **release_as**. Leave the input empty for normal Conventional Commit version calculation. The override path uses Release Please's manifest-aware CLI so the package and every configured extra version file remain synchronized. The manual run creates or updates the release pull request; it does not publish the release until that pull request is merged.

## Retrying a build

If an artifact job fails, first rerun the failed jobs in that GitHub Actions run. To rebuild an existing draft later, manually run **Build release** and enter its tag, such as `v0.2.0`. The workflow checks out the tag, rejects mismatched application versions, and uploads to the matching draft release.

For a historical release whose only mismatch is SoundGrammy's own version in `Cargo.lock`, enable **repair_cargo_lock** on the manual run. This runs a targeted `cargo update --package soundgrammy` in the build workspace before validation; it does not move the tag or modify the repository. Normal automated releases leave this disabled so a bad Release Please update still fails visibly.

Do not create version tags by hand during the normal workflow. A manually created tag can get ahead of the source manifests and confuse the next release calculation.

## Local version check

Run this before reviewing or troubleshooting a release:

```bash
bun run check:versions
```

For an expected tag:

```bash
EXPECTED_VERSION=v0.2.0 bun run check:versions
```

The manifest in `.release-please-manifest.json` records the current release baseline. The next Release Please pull request calculates its bump from Conventional Commits after the corresponding tag.
