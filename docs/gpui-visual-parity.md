# GPUI visual comparison

> Historical migration evidence. The [current audit](gpui-audit.md) supersedes development commands, baseline references and test counts below.

Reference: the installed macOS SoundGrammy 3.5.0 app, inspected live on 2026-11-06, with `v3.5.0` sources used to resolve layout and interaction details. The initial migration's generic modal/player layouts were replaced after the user's visual review. This document records the follow-up comparison rather than treating automated tests as proof of visual equivalence.

## Reference conditions

The desktop window was 1200 × 800 logical pixels, with a 32-pixel native title bar and a resizable sidebar. Narrow-window checks used macOS Window → Move & Resize → Left (approximately 757 × 949). The installed app was used for navigation and inspection; account mutations and playback controls were avoided during the comparison.

A temporary account-isolated presentation fixture contained the same 460 tracks, 439 cached artwork paths, three likes, nine Presentation tracks and 449 Popular tracks. It carried no Telegram document descriptors, credentials or encrypted session material. It reproduced a paused queue position to make screenshots comparable. It is not committed or bundled. The real account was also opened in the rebuilt packaged native app after the user's Keychain approval. A short cached Synthetic playback advanced to 0:16; pause and the 460-entry queue were verified. The app was left open and paused for review. This is playback-state evidence, not an independent audible-output measurement.

## Layouts compared and changes made

| Surface | Native implementation and comparison points |
|---|---|
| Desktop library | 32-pixel toolbar controls, 36-pixel table header, 70-pixel rows on a 78-pixel stride, 48-pixel artwork; proportional title/artist columns, fixed time/options widths; active artwork blur, blue active title and play indicator. |
| Sidebar | Original gradient/collage covers, playlist counts, hover options, 52-pixel desktop and 72-pixel compact rows. The minimum-width header remains on one line. |
| Player bar | 96-pixel bar; 64-pixel desktop/56-pixel compact artwork; original control spacing, filled transport icons, repeat/shuffle states, white liked heart, buffered seek/loading indication, hover volume and exact volume entry. Desktop artwork opens fullscreen; compact description opens the player drawer. |
| Navigation | Left 320-pixel drawer, account avatar/name/username, sync status, settings, logout and version footer. Modal background blur reads only the application's own Metal frame on macOS; it does not request screen-recording access. |
| Settings | Centered 512-pixel dialog with five initially collapsed disclosures; cache card and paired fields, listening statistics, Last.fm/proxy integration, diagnostics and fullscreen/bounce controls. Expanded forms scroll within the original viewport limits. |
| Queue | 320 × 450 popover above the player at the right edge; source and position, history/current/up-next sections, artwork, row actions, collapse and overflow menu. Saving supports full queue, from here and up next, preserving positional duplicates. |
| Playlist dialogs | Original New/Import tabs, descriptions, name field, native file selection, content-width import button and footer. Blank names disable creation. Rename and save-queue use the same layout family. |
| Details and jobs | Track artwork plus listening/track/document/attribute cards, diagnostic log viewer, confirmations and queued download-result dialogs. |
| Compact layout | Single-row playlist header, larger actions/artwork, stacked title/artist, full-height player drawer with responsive artwork/controls. Opening queue over the player preserves the drawer underneath. |
| Fullscreen | Centered artwork with border/shadow, blurred palette background and slow drift, original metadata/control arrangement, bounce preferences, idle hiding and Escape restoration. |
| Sign-in/indexing | Original narrow auth shell, QR/phone/code/password layouts and indexing progress; native text input remains underneath the five code cells for paste and IME support. |

## Regression evidence and limits

### Interaction audit after the second user review

The follow-up audit uses the installed 3.5.0 app and its tagged source as the behavior reference. It adds 12 production-view regressions and strengthens the existing volume, queue placement and compact-player checks. The full native suite passes 350 tests (one pre-existing ignored test), including 46 GPUI view tests.

| Report | Correction and verification |
|---|---|
| Invented playlist option | Removed “Open playlist”; All tracks has no options trigger or context menu. Regression checks both entry points. |
| Different dropdown | Original action order, group labels, widths, icons and playlist submenu/create action restored. Native GPUI menus retain keyboard/submenu handling. |
| Broken seek bar | Base Slider keeps a continuous 4px visual track inside a larger pointer target. Hover time uses the measured track bounds and a deferred Base Tooltip positioner; hover does not issue a seek. |
| Volume vanishes during pointer travel | Base HoverCard owns delayed closure across trigger and popup. The popup's outer hitbox owns occlusion. A timed pointer-travel regression crosses the gap, waits beyond the close delay and edits exact volume. |
| Queue wheel scrolls library | Base Popover owns popup hit testing; wheel propagation stops at the queue. Regression verifies queue offset changes while library row bounds stay fixed. |
| Empty queue popup remains | Empty queue snapshots close queue and compact-player surfaces, including stacked panels. |
| Different drag feedback | Native GPUI drag renders a full row at its original width, hides the source, shifts neighbours vertically and keeps collision bounds stable. Reordering is restricted to eligible lists. Regression performs a real pointer gesture, verifies the target index and cancels with Escape without mutation. |
| Different bulk selection | Restored Actions/count and Exit toolbar, mixed Select all indicator, row checkbox and subtle selection border/background. Playback actions are replaced; row menus are removed while selecting. |
| Modal click-through | Base Dialog owns dismissal and event interception. Backdrop no longer occludes its parent's dismissal handler. Regression clicks over an underlying track and verifies dismissal without a playback command. |
| Filter trigger reopens | Base Popover owns trigger toggling, outside press, Escape and focus return. Regression opens and closes using the same trigger. |
| Static sort icons | Primary and secondary columns render ascending, descending or unsorted arrows. Regression verifies icon changes across the sort cycle. |

The adjacent audit also corrected playlist export changing the active playlist, bulk Add to Liked toggling already-liked tracks off, hidden row-menu hit targets during selection, playlist dialog initial/return focus, and keyboard activation/navigation for track rows, playlist tabs and filter choices. Settings disclosures now use Base Collapsible; compact playback uses Base Sheet; the library uses Base Table semantics.

The final macOS release binary was inspected live in the isolated fixture: track menu and bulk selection screenshots; ascending/descending/unsorted icons and corresponding row ordering; navigation dismissal over a track without changing the player; volume trigger-to-popup travel and exact-input opening; centered seek hover time without changing playback position; queue anchoring, queue-only wheel scrolling and clear-to-close; filter trigger toggling; playlist-name initial focus; and settings layout/outside dismissal. The fixture has a presentation-only paused queue, so commands that touch the isolated empty audio engine can replace that preview queue; this is not used as real-account playback evidence. The packaged real-account app is checked separately.

After packaging, the updated real-account app reopened successfully with all 460 library tracks, 449 Popular tracks, three likes and the nine-track custom playlist. Its paused player displayed confessions / Bad Computer with finite seek progress and artwork. It was left open for user review. The macOS bundle/DMG and Android debug APK passed their signature/checksum checks after this UI revision.

### Component inventory and gaps

The pinned GPUI Base Kit 0.8.1 supplies Dialog, Popover, HoverCard, Slider, Tooltip, Checkbox/Indicator, Radio, Tabs, Collapsible, Sheet, Button, Input, Link and Table primitives. Those own their corresponding interactions; application styling preserves SoundGrammy's existing design.

Base Kit 0.8.1 has **no headless Menu or sortable-list primitive**. This was reported during the work. Menus use the existing native `gpui-component` menu/dropdown/context-menu facilities, with SoundGrammy styling. Sortable lists use GPUI's native drag events with explicit positional guards and cancellation. The original app and official React Base UI [Menu](https://base-ui.com/react/components/menu), [Popover](https://base-ui.com/react/components/popover) and [Dialog](https://base-ui.com/react/components/dialog) documentation supply the fallback behavior reference. No webview or React runtime was introduced.

The pinned Tabs/Radio primitives need application arrow-key navigation; Dialog needs application return-focus tracking. Those integrations are tested. Although Base Kit provides an OTP presentation, sign-in retains native Input underneath the visual cells so paste, IME and mobile editing remain available.

### Playback and motion follow-up (2026-11-08)

- Restored Base Kit's measured `SliderIndicator` to the seek and vertical volume rails. Without it, pointer positions divided by a zero track size and could produce `NaN`; UI synchronization could also overwrite input during redraw. The seek rail now starts at the player boundary, with a larger invisible hit area. Both controls reject non-finite values before dispatching an audio command.
- Base Kit Presence supplies entry and exit timing for dialogs, queue and filter popovers, the volume hover card, and the navigation and player drawers. Closing visuals are brief and separate from live popup behavior. The active track shows the original three animated bars, with a still variant for reduced motion. The pinned component dropdown/context-menu implementation has no transition hook and retains its native dismissal.
- Search tracks uses the native Input clear affordance in both layouts. Clicking it clears the query and restores the library rows.
- Live macOS verification on the packaged app: clicked 25% and dragged to 60% of a paused track, landing at 0:48 and 1:57 of 3:15. Sought to the midpoint of a playing cached track; time advanced from 1:38 to 1:45 and the equalizer bars changed between frames. The volume rail changed 28% to 66% with the popup still open, then restored 28%. The search clear button restored the unfiltered library; the playlist dialog opened and dismissed with Escape.
- Five new regressions cover seek geometry and pointer targets, volume interaction, search clearing, modal/popover exit, and equalizer motion with reduced-motion support. The full suite passed **355 tests** (one pre-existing ignored), including 51 GPUI view tests. The macOS app/DMG and Android debug APK were rebuilt and verified.

### Gesture and fullscreen follow-up (2026-11-08)

- The seek rail's extended mouse region now blocks underlying row hover and clicks while still allowing wheel scrolling. The regression presses where the seek target physically overlaps a library row and checks the ordered playback port receives only a seek.
- The pinned HoverCard suppresses hover during a GPUI drag and has no gesture pin. Volume now uses Base Popup placement and the same Base Slider, with dismissal held until mouse-up. The regression keeps a drag outside the popup for 500 ms, drags back to 80%, releases inside, and verifies closure after the pointer leaves.
- The original fullscreen operations panel waits 3 seconds, then slides 16 pixels and fades over 500 ms with the original easing. The exit button fades separately. Pointer movement over a fading control can reveal the panel again. The artwork and exit buttons use the original Maximize2 and Minimize2 glyphs.
- Player tooltips use the original action labels instead of icon names. Managed button tooltips dismiss without the old grace period; the seek tooltip opens without a hover delay, shows the original time format, and uses a short entry fade.
- Artwork animation runs in a separate GPUI view, with the original 750 ms profile crossfade, attack/release, scale and lift. Its playback clock is reanchored only when the track, position or playing state changes. Track changes reset bounce energy, and a late high-quality cover replaces the ordinary thumbnail even if the ordinary request finished first. The regression samples the artwork during quiet and loud parts of a profile, on pause, and under reduced motion.
- Live on macOS, a click above the rail sought to 1:22 with the same track still playing and no error. The final release binary also opened fullscreen in the isolated preview with the cover, controls and exit glyph visible. The long volume gesture, exact fade timing and audio-profile response were verified in native view tests. The packaged app was left running for review.

Production GPUI view tests cover desktop/compact layout and interactions, not a mock HTML reconstruction. They complement the live rendered comparisons. Service tests retain audio/session/cache/playlist ownership, cancellation, account epoch and ordered-command checks.

Screenshots differ when the two apps have different queue state, errors, hover/focus, animation frame or OS window state. Native font rasterization and AppKit chrome are not certified by an automated pixel-diff threshold. The macOS comparison does not certify Android, Windows or Linux rendering. Android hardware acceptance was explicitly deferred by the user; those remaining platform checks are listed in [migration acceptance](gpui-migration.md#acceptance-still-requiring-another-environment).


## Player footer, empty playlists and hover titles (2026-11-16)

The compact player and its cached wrapper both reserve 80px (96px on desktop), removing the unused strip below the controls. Genuine empty playlists omit playback/storage/selection actions and the table, and show the original context-specific empty-state copy. Compact navigation and search remain available. A search returning no matches retains the playlist toolbar and table because playlist membership still exists.

Descriptive hover titles use GPUI's cursor-positioned, non-hoverable tooltip primitive with a 1.5-second delay, compact styling and viewport-aware placement. They reveal complete playlist names, track titles and artists, account/source text, import filenames and playback control descriptions. Immediate seek feedback remains separate. A shared lifetime token invalidates visible and pending titles on panel transitions, fullscreen toggles and account resets; no polling or animation loop is added.

Six additional deterministic native regression tests cover footer bounds across compact/desktop widths, the five empty playlist cases in both layouts, filtered empty results, delayed volume titles and pointer travel into the exact input, full-name titles, and visible/pending title dismissal when opening a drawer. These tests use the account-free preview fixture and do not launch the production app.


## Fullscreen cursor follow-up (2026-11-16)

On macOS, the cursor now hides at the same three-second inactivity threshold as fullscreen controls. Pointer movement, keyboard input and window activation reveal it again; open panels, the volume popover and active seek drags keep it visible, and leaving fullscreen restores it. Cursor visibility and controls share one predicate and the existing idle timer. The local macOS backend exposes a GPUIView selector using AppKit's hide-until-mouse-moves mechanism and updates GPUI's visibility tracking together; it does not use the globally balanced hide/unhide counter or add a polling loop. This native bridge is specific to macOS; other desktop backend cursor hiding remains outside this platform's acceptance evidence.

Deterministic native-window tests cover the three-second boundary, pointer and keyboard restoration, interaction hold-open states and exiting fullscreen. The tests assert the application's requested cursor state; their account-free headless windows do not invoke AppKit's actual cursor API.
