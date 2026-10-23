# The mini player as a complete Spotify client

Status: draft for review, 2026-10-22. Fork only (`ckearl/spotifast`); not
intended for upstream, so the upstream rules about opening an issue first and
not adding settings do not bind it. The architecture rules in `AGENTS.md`
still do.

## Why

The owner tried Spotifast's Winamp mini player and fell for it: the classic
look, the skins, and how small it is. They want it to be their primary
Spotify app, replacing `spotify_player` (a terminal client they used daily
for one to two years). Today the mini player sends them back to the big
window for search, lyrics, browsing their library, recently played, and
choosing a device. The goal is that they never have to leave it for
everyday listening.

**Success:** a full day of normal listening without opening the main window.

**Anti-goals (failure even if it works):**
- It stops feeling like Winamp: modern-looking widgets, egui-styled panels,
  anything that breaks the skin.
- It gets cluttered, or needs a big window to be usable. Every new part is
  optional and closed by default.
- Anything the user does flickers or reverts (see the optimistic-UI rule in
  `AGENTS.md`).

## Goals

1. **FIND**: search, library, and recently played, in one new section.
2. **Browsing**: picking an artist, album, playlist or radio lists its songs
   inside FIND, with a way back.
3. **Actions** on songs and on artists, albums and playlists, by mouse and by
   keyboard.
4. **LYRICS**: a new section that follows the playing song.
5. **VIS**: a new section with a wide bar-graph visualizer, like
   `spotify_player`'s.
6. **Devices**: pick the playback device from the mini player.
7. **Keyboard**: everything above is reachable without the mouse.

## Non-goals

Browse/Home pages, sorting, podcasts, settings, the logs page, friend
activity, editing playlists beyond add/remove. These stay in the main window.
Winamp 3/5 skins remain unsupported.

## Constraints

- Everything is drawn in the skin: frames from `pledit.bmp`, list colours from
  `pledit.txt`, text through `pixel_text` (as the playlist rows are), bar
  colours from `viscolor.txt`. No egui widgets inside the sections, except
  the existing style of popup menu (`super::menu`), which the mini player
  already uses for ADD/REM/SEL/MISC/LIST OPTS.
- Classic skins have no art for these sections. Build them from the playlist
  frame pieces (corners, edge tiles, title tile with its baked text blanked)
  and draw the section name with the skin's font. They must look native in
  any skin, not only the default.
- Views only emit `Action`s; the backend does network work (`AGENTS.md`).
  Reuse existing actions and loaders wherever they exist (table below).
- Queue changes obey `docs/_reference/queue.md`.
- Each section integer-scales with the rest of the window (1x–4x), keeps its
  open, shaded and height state across restarts, and survives switching to
  the main window and back, like EQ and PL do now.

## What already exists to build on

| Need | Existing |
|---|---|
| Search | `Action::Search`, `SearchState` (`src/model.rs`) |
| Library | `Page::LikedSongs`, `Page::Albums`, `Page::Artists`, the playlists list behind LIST OPTS |
| Recently played | `app.recently_played` (local history merged with Spotify's, `src/history.rs`) |
| Album / artist / playlist / radio songs | `Page::Album`, `Page::Artist`, `Page::Playlist`, `Page::Radio` with `App::ensure_loaded` |
| Queue | `AddToQueue`, `PlayNext`, `QueueMany`, `RemoveFromQueue` |
| Playlists | `AddToPlaylist`, `RemoveFromPlaylist`, the submenu in `src/ui/widgets.rs` |
| Like / save / follow | `ToggleSaved`, `SetSavedMany`, and the follow/save actions the main pages use |
| Lyrics | `App::request_lyrics`, `app.lyrics`, synced `Line`s (`src/lyrics.rs`) |
| Visualizer | `vis::WideAnalyser` (75 bands, already drives the main player bar), `viscolor.txt` parsing |
| Devices | `Action::Transfer`, `RefreshDevices`, the picker data behind `ToggleDevicesPopup` |

## Layout: stack what you want

The owner chose this over a single tabbed drawer. New sections stack under
the existing ones, in this order: main, EQ, PL, FIND, LYRICS, VIS. Each has
its own toggle, close button and shade button, like EQ and PL; each is
closed by default.

Toggles: the main window gains FIND, LYR and VIS buttons beside EQ/PL, plus
a DEV button that opens the device menu. *(Builder's call: exact placement
and art. Classic skins' main window has room only for EQ and PL in the
art, so the new buttons will likely be drawn with the skin font in the
clutter-bar style or live in the O menu plus keyboard; pick what looks
native and say what you picked.)*

FIND and LYRICS are resizable in the playlist's 29-pixel steps; VIS has one
fixed height *(my call: about 58 skin pixels)*. If the stack would be
taller than the screen, sections keep their size and the window is allowed
to extend; nothing auto-closes *(my call)*.

## FIND

From top to bottom:

1. **Input bar**, drawn with the skin font, with a caret, and a clear (X) control.
   Typing updates results the way the main search does (debounced through
   the existing `SearchState`; sharing it with the main window is fine).
2. **Mode row.**
   - While the box is empty: **LIBRARY** and **RECENT**.
   - While searching: **SONGS**, **ARTISTS**, **ALBUMS**, **LISTS**
     (playlists).
   - LIBRARY uses the same four kinds: Liked Songs, followed artists, saved
     albums, your playlists.
3. **List**: up to 15 rows for search results, and the full list for library
   and recent, scrollable with a skin-style scrollbar. Rows are numbered like
   PL. A selected row uses the playlist's selection colours, and the playing
   song uses its "current" colour.
4. **Bottom bar**: PLAY, +QUEUE, +LIST, RADIO, BACK *(labels are my call;
   art borrowed from the PL bottom bar)*.

**Choosing a row** (double-click or Enter):
- Song: plays it, in its context where one exists (the album, the playlist,
  the artist's top songs). A song from search or recent plays on its own and
  Spotify's autoplay carries on after it *(my call)*.
- Artist: lists their top songs, with a second mode row (SONGS / ALBUMS) to
  reach their albums.
- Album or playlist: lists its songs.
- RADIO on any row: lists that row's station (`Page::Radio`) the same way.

Each drill-down pushes onto a back stack. BACK, or Backspace while the box is
empty, pops it. The title line shows where you are, for example
`FIND · In Rainbows`.

## Actions

Wherever a row is selected (FIND, PL, and the playing song) the same set of
actions applies. Right-click opens them as a menu; the keyboard reaches them
too (below).

- **Song:** play, play next, add to queue, add to playlist ▸, like/unlike,
  remove from this playlist (when inside one you own), go to artist, go to
  album, radio, copy link.
- **Artist:** play, follow/unfollow, radio, copy link.
- **Album / playlist:** play, add to queue, save to / remove from library,
  radio, copy link.

"Go to artist/album" opens it inside FIND (opening FIND if it is closed). It
never jumps to the main window.

## LYRICS

- Shows the current line in the "current" colour, with the lines around it
  in the normal colour, centred.
- Synced lyrics follow the song. A scroll pauses following, and following
  resumes on the next track change or on a click *(this mirrors the main
  panel's `FollowLyrics` / `PauseLyricsFollow`)*.
- Clicking a synced line seeks to it *(my call)*.
- Unsynced lyrics scroll freely. "No lyrics" and errors show as a single
  line, and clicking an error retries.
- While the section is open, it calls `request_lyrics` on every track
  change. Right now only the main panel asks for lyrics, so this hook is
  required.

## VIS

- A wide bar graph driven by `vis::WideAnalyser`, coloured from the skin's
  24 `viscolor.txt` entries from bottom to top, as Winamp's analyser was,
  with falling peak marks.
- It follows the house rule: post-equalizer, pre-volume, and still moving at
  zero volume.
- When playback is on another device it shows a flat line and the message
  `playing on <device>`, rather than disappearing.

## Devices

DEV opens a menu in the existing mini-player menu style. It lists this
computer and every Connect device, with a check on the active one. Choosing
one transfers playback to it (`Action::Transfer`). The device's name also
shows briefly in the marquee.

## Keyboard

The single-letter keys work whenever the mini player has focus and you are
not typing. Typing in FIND keeps every letter. Arrows, Enter, Esc, Tab and
Backspace drive FIND while its box is focused.

**Sections**: one key each, matching what the main window uses where it can.
- `/` or `Cmd/Ctrl+F` focuses FIND, opening it first if needed.
- `e` EQ, `q` PL (the queue, as `q` is in the main window), `l` LYRICS,
  `v` VIS.

**Two-key sequences** in the style of `spotify_player` and tidied up. Each
starts with a verb: `g` go, `a` add, `d` remove, `y` copy. The first key
shows in the marquee (`g…`) until the second arrives or a second passes.
Sequences act on the selected row when a list has focus, and otherwise on
the playing song.

| Keys | Does |
|---|---|
| `g a` | go to artist |
| `g b` | go to album |
| `g r` | radio |
| `g h` | recently played (history) |
| `g y` | your library |
| `g d` | devices menu |
| `g p` | show the playing song in its context |
| `a q` | add to queue |
| `a n` | play next |
| `a p` | add to playlist ▸ (the menu opens; arrows and Enter pick) |
| `a l` | like / save / follow, whichever fits the item |
| `d l` | unlike / unsave / unfollow |
| `d p` | remove from this playlist |
| `d q` | remove from queue |
| `y` | copy link |

The refinements over the owner's `spotify_player` keymap:
- Like, save-to-library and follow are one verb (`a l` / `d l`), so you don't
  have to remember which kind of item takes which key.
- Copy link is one key (`y`, as in "yank") instead of `c p u`.
- Album is `g b`, not `g B`, so no Shift is needed.

Space, `m`, `s`, `r`, `b` (like the playing song), the arrows with modifiers,
and `?` keep their current global meanings. `?` must list the mini-player
keys. Classic Winamp's Z X C V B transport keys are left out on purpose,
because they collide with `v`, `b` and the verbs above *(my call)*.

## Left to the builder

- Exact pixel layout, button art and label wording within the skin.
- How results and pages are cached and refreshed (reuse the main window's
  loaders).
- Size and scroll limits beyond those stated above.
- Test strategy. At minimum, follow the existing module tests: FIND state
  (modes, back stack, choosing), key sequence parsing and timeout, LYRICS
  following, and the stack height maths in `src/ui/winamp/mod.rs`.

## Suggested order

Each step is usable on its own:

1. Section framework: generic skinned section, stacking, persistence.
2. FIND with search and the action menu.
3. Library, recent and drill-down.
4. Keyboard.
5. LYRICS.
6. Devices.
7. VIS.
