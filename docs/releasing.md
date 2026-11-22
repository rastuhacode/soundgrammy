# Releasing SoundGrammy

SoundGrammy uses [Release Please](https://github.com/googleapis/release-please) to turn Conventional Commits on `main` into a versioned release pull request, changelog entries, a Git tag, and a draft GitHub Release. The Tauri build workflow then uploads macOS, Windows, and Android arm64 artifacts to that draft.

## Workflow

1. Merge changes into `main`, preferably with squash merge and a Conventional Commit title.
2. The [Release Please workflow](../.github/workflows/release-please.yml) opens or refreshes a release pull request. It proposes the next version, updates `CHANGELOG.md`, and synchronizes the package, Tauri, Cargo, and release manifests. The workflow also commits the derived `-develop` version to `src-tauri/tauri.dev.conf.json`.
3. Continue merging ordinary changes. Release Please keeps the same release pull request current.
4. Review the generated changelog and version changes, then merge the release pull request when the release is ready.
5. Release Please creates the `vX.Y.Z` tag and a draft GitHub Release. In the same run, [Build release](../.github/workflows/release.yml) checks out the exact release commit, verifies versions, and uploads Intel and Apple Silicon macOS bundles, Windows bundles, and a signed Android arm64 APK.
6. Test the uploaded artifacts and edit the release notes if needed. Publish the draft GitHub Release when it is ready for users.

Publishing is manual. Use the [platform playback checklist](platform-playback.md#device-verification) when testing release artifacts on devices.

## Commit titles and version bumps

Release Please derives SemVer changes from commit messages:

| Commit title | Result |
| --- | --- |
| `fix: prevent stalled playback` | Patch release |
| `feat: add queue export` | Minor release |
| `feat!: replace playlist recipe format` | Major release |
| `docs: clarify setup` or `chore: update tooling` | May appear in notes but does not trigger a version bump by itself |

Use a `Release-As: X.Y.Z` footer on a commit only when an explicit version override is required. Prefer squash merging pull requests so the final pull request title becomes the single release-note-worthy commit on `main`.

To override the next version without adding a commit, manually run **Release Please** and enter the desired SemVer value in **release_as**. Leave it empty for normal Conventional Commit calculation. The manual run creates or updates the release pull request. Merging that pull request triggers the draft build and publishing remains manual.

## Retrying a build

If an artifact job fails, first rerun the failed jobs in that GitHub Actions run. To rebuild an existing draft later, manually run **Build release** and enter its tag. Leave `sha` blank to build the tagged commit. The workflow validates application versions and uploads to the matching draft release.

For a historical release whose only mismatch is SoundGrammy's own version in `Cargo.lock`, enable **repair_cargo_lock** on the manual run. This runs a targeted `cargo update --package soundgrammy` in the temporary build workspace before validation. Automated releases validate the lockfile as committed.

Do not create version tags by hand during the normal workflow. A manually created tag can get ahead of the source manifests and confuse the next release calculation.

## Local version check

Run this before reviewing or troubleshooting a release:

```bash
bun run check:versions
```

For an expected tag:

```bash
EXPECTED_VERSION=v3.5.0 bun run check:versions
```

The manifest in `.release-please-manifest.json` records the current release baseline. The next Release Please pull request calculates its bump from Conventional Commits after the corresponding tag.
