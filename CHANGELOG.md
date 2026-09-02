# Changelog

## [2.0.0](https://github.com/rastuhacode/soundgrammy/compare/v1.2.0...v2.0.0) (2026-09-02)


### ⚠ BREAKING CHANGES

* lots of streaming enhancements - lookahead, ID3 lazy loading, etc.

### Features

* add DeadSpace component for funsies ([27d8c5f](https://github.com/rastuhacode/soundgrammy/commit/27d8c5f8b5c1aced3de7fe79b7b4f7616ec3249a))
* add experemental last.fm scrobbling ([3d4601f](https://github.com/rastuhacode/soundgrammy/commit/3d4601f2f447d1f1fb3a3c56c08fe1cd384f60b4))
* add missing titles ([aff8adb](https://github.com/rastuhacode/soundgrammy/commit/aff8adbbeeffeb266e6f7a8a255eef6f064555d1))
* enhance development environment with isolated app identity and configuration ([887e25d](https://github.com/rastuhacode/soundgrammy/commit/887e25d3c2ddbfa1960264dcdb5acc9382e441b7))
* implement caching and request coordination for Telegram media downloads ([b978b36](https://github.com/rastuhacode/soundgrammy/commit/b978b367e62ba02eab17f46ff17a5c74b6e644e3))
* lots of streaming enhancements - lookahead, ID3 lazy loading, etc. ([8aa7111](https://github.com/rastuhacode/soundgrammy/commit/8aa7111b4c45e84aea151435a9171e58467b6b2b))
* replace SidebarProfile with SidebarDrawer for improved navigation and user experience; add new TauriLink component for external links; implement AlertDialog and Drawer components for better UI interactions ([87791f3](https://github.com/rastuhacode/soundgrammy/commit/87791f3d3f29c0a09013923ab8c12af58a667c94))


### Bug Fixes

* fixed missing avatar on initial launch ([cbd7524](https://github.com/rastuhacode/soundgrammy/commit/cbd7524730af910ffee7bde6f2a26729634f072b))
* prevent proactive removal of MP3 back buffer during normal playback to avoid playback stalling in WebKit ([9553d07](https://github.com/rastuhacode/soundgrammy/commit/9553d07f1a5d67694b8c62f17802eff86298e601))

## [1.2.0](https://github.com/rastuhacode/soundgrammy/compare/v1.1.0...v1.2.0) (2026-08-27)


### Features

* add fullscreen display wake functionality to prevent screen dimming during fullscreen playback ([bb232c9](https://github.com/rastuhacode/soundgrammy/commit/bb232c9eab7088f25ce1aa999fcf7f2d8573a819))
* enhance media session controls with track metadata and playback position management ([968e8e6](https://github.com/rastuhacode/soundgrammy/commit/968e8e617900a662f796d1f09eefb824a81676bc))

## [1.1.0](https://github.com/rastuhacode/soundgrammy/compare/v1.0.0...v1.1.0) (2026-08-20)


### Features

* add arrows keybind to rewind on 5 sec ([ed7ced4](https://github.com/rastuhacode/soundgrammy/commit/ed7ced4c4ad1542bd438457a9607140cdd2c0b67))
* add device events integrations, e.g. pause from headphones ([77a5a5e](https://github.com/rastuhacode/soundgrammy/commit/77a5a5e2481c82e15bb0117eab8408adf41b81d3))
* added keybinds for pause, next, previous tracks ([8753069](https://github.com/rastuhacode/soundgrammy/commit/8753069f4eafc2ec6c1d388fd77f93d370f05829))
* refined shuffle button ([d01fade](https://github.com/rastuhacode/soundgrammy/commit/d01fadee5096d5f4d1bfa705238dd875e0e228a6))


### Bug Fixes

* app doesn't load infinelly if connection through proxy is turned off ([d0d8d27](https://github.com/rastuhacode/soundgrammy/commit/d0d8d27bd8a639ac7bce65c83ebc356382816e8a))
* make playlist name autofocused on create / edit ([d8551cb](https://github.com/rastuhacode/soundgrammy/commit/d8551cb12c7bbb7f6cd4f43eedde487f3e75dec4))

## [1.0.0](https://github.com/rastuhacode/soundgrammy/compare/v0.4.0...v1.0.0) (2026-08-11)


### Features

* add context menu button to PlaylistTrackRow for track options and update table structure to include actions column ([1477ec9](https://github.com/rastuhacode/soundgrammy/commit/1477ec978735c1d8db39abbef161182839c57af0))


### Bug Fixes

* (experement) remove forward buffering logic from MSE session management to simplify playback handling ([7f15a18](https://github.com/rastuhacode/soundgrammy/commit/7f15a1836f3aea09bfadfb828e72b0de07fdd128))

## [0.4.0](https://github.com/rastuhacode/soundgrammy/compare/v0.3.0...v0.4.0) (2026-08-11)


### Features

* enhance MSE session management with quota handling and forward buffering logic ([c05a824](https://github.com/rastuhacode/soundgrammy/commit/c05a824bc0695e6de34f4c25c21c65165601767a))

## [0.3.0](https://github.com/rastuhacode/soundgrammy/compare/v0.2.0...v0.3.0) (2026-08-10)


### ⚠ BREAKING CHANGES

* streamline playlist management by removing thumbnail handling and removed migration functions

### Features

* enhance shuffle functionality with new modes and UI components for improved user experience ([5f2255b](https://github.com/rastuhacode/soundgrammy/commit/5f2255bd220bc0d2f38f7ea40c885d038d27fa2c))
* implement listening statistics setting with enable/disable and clear ([fde8740](https://github.com/rastuhacode/soundgrammy/commit/fde874097865d409d5fff9c2d6800f00af17cb4d))
* implement logging system with diagnostic logs and error handling for improved debugging ([4e3bd51](https://github.com/rastuhacode/soundgrammy/commit/4e3bd5187fef290a6025e1dfc63b2ab95f727795))


### Code Refactoring

* streamline playlist management by removing thumbnail handling and removed migration functions ([a48a6cc](https://github.com/rastuhacode/soundgrammy/commit/a48a6ccf48f847a16a4960d0ad2515d3cae18e91))

## [0.2.0](https://github.com/rastuhacode/soundgrammy/compare/v0.1.1...v0.2.0) (2026-08-06)


### Features

* add GitHub Actions workflow for releasing with WebView DevTools and enhance MSE session error handling ([d05cf12](https://github.com/rastuhacode/soundgrammy/commit/d05cf123b4f611e003f4e8d8355291342e5ceab1))
* enhance PlaylistTrackRow and PlaylistTracksTable components with improved drag-and-drop functionality and sortable track IDs ([b539981](https://github.com/rastuhacode/soundgrammy/commit/b539981856e04418311320fa5b1d083807838878))
* implement fullscreen artwork bounce feature with adjustable settings and profile analysis ([b16b02e](https://github.com/rastuhacode/soundgrammy/commit/b16b02e8b179f3831b52fb963a62112db7ef6089))
* integrate react-resizable-panels for improved layout management in App component and add custom Slider component for volume control ([c870fd9](https://github.com/rastuhacode/soundgrammy/commit/c870fd9a4a99e3282e108ef133775a5d81c529d6))
* refactor volume handling in audio components to use numeric values and improve user interaction ([64c4204](https://github.com/rastuhacode/soundgrammy/commit/64c4204111ca86ef844d9fb28f11eadee89ab590))
* restrict drag-and-drop functionality to vertical axis in PlayerSidebar component ([d69bb57](https://github.com/rastuhacode/soundgrammy/commit/d69bb5750f3146f5eb229294b66f60b0f484fd67))
* update version to 0.1.0 and enhance SidebarProfile with detailed sync status, error handling, and manual sync functionality ([7ba9a97](https://github.com/rastuhacode/soundgrammy/commit/7ba9a9781fa49b61764ad57293c57542e15a6255))


### Bug Fixes

* adjust minimum size of playlist sidebar panel from 240 to 220 for better layout flexibility ([dc5ae12](https://github.com/rastuhacode/soundgrammy/commit/dc5ae12b97327129acc8b96574f65435a4691971))

## Changelog

Release Please maintains this file from Conventional Commit messages merged into `main`.
