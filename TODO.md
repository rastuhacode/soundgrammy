General ideas what should be implemented
Main targets on release - macos and windows. Mobiles added a bit later.

## Essential

1) Table in playlist tracks, sorting the tracks, mass selection for playlist creation or add to liked, moving tracks in playlists (everywhere except "All tracks")
2) Add "pin" and "hide" options for playlists. Hide should remove plylist from the list (primarly for default ones - liked, etc). Pin should pin any playlist to top - it still would be filtered with search and it would have their own order with dnd (like currently in custom order list)
3) Add queue representation in audio player, make it editable (move, add, remove tracks, create playlist from queue - would be cool with smart shuffles playlists or some advanced filtering (by weights or genres))
4) Track cache: cache whole playlist, cache track without playing, clear cache, visual of cache staus (maybe load button like in telegram)
5) Check on how to work fully offline, currently it seems like the app checks session validation on launch, which makes it unavaliable offline. Need to find some practices / middle ground how we could achieve both secure sessions and offline work (maybe look how original telegram desktop works).
6) Playlists: import and 2 level export. first level is to export json to then import in another device. As alternative load this json to some tg channel (like Saved messages - private, only user storage) and then fetch it. Second level of import is to export m3u file (idk maybe with original files or not, should be thought through more).
7) Killer feature i would want to implement - playlist share. The user A can export json and other user B (other account) import it. for shared tracks it would be nescessary to be in A's saved tracks and their saved music should be visible to user B. The issue is that it breaks the "All tarcks" (saved music) as general immutable source for all playlists.
-- Share can't be done as all files needs to have FileSource which are controlled by user who exported songs. If they would remove this tarcks - they also would be removed in playlist another user imported, which would lead to frustration. The only way to create user's file reference is to save to Profile, which would pollute All tracks inevitably. The only option is to "Live share" (subscribe) to playlists with exposing "clone" which would require to add missing tracks to all tracks, which is questionable.
Also current "issue" is how react in playlists if the track was removed from All tracks (e.g. user removed track in telegram app).
8) Add user's listen statistic, make "most popular" playlist (give weights to tracks based on listening) and other such cool stuff. Add auto playlist generation (maybe llm by genres, date, by popularity (weights > X))
8) Add auto mtproto proxy (to avoid Russian blocks) with options to turn on in settings 


## Tolerable

1) Add compressor to fullscreen (the thumbnail must bounce to transients of the track)
2) Add settings page (remove bounce, clear cache, etc. will be extensible later with other features)
3) Add different shuffle algorithms (smart, etc.)
4) Add folders with playlists (basically tree in sidebar)
5) Add adaptivity (nessescary for Android, Ios integration)
6) Smart search, as an idea make search by keywords / commands, for ex. "@author:Skrillex || @autor:Juelz", "@song:Bangarang && @genre:Dubstep" "@tag:workout,chill && @weight:>10". Maybe some sanitized sql requests.

## Nice touches

1) Different list presentations for playilists and tracks (cards, without thums list, etc)