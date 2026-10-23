# Mini Player Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Winamp mini player a complete everyday Spotify client by adding FIND (search, library, recent, drill-down), item actions, a keyboard layer, LYRICS, VIS, and a device menu, all drawn in the skin.

**Architecture:** Three new stackable sections (FIND, LYRICS, VIS) hang under PL, each drawn with a shared skinned-section frame built from `pledit.bmp` pieces. FIND's logic (modes, back stack, rows, what choosing does) is a pure state module with no egui, so it can be unit-tested; drawing modules only read `App` and push `Action`s. Every Spotify operation reuses an existing `Action` or loader.

**Tech Stack:** Rust (toolchain pinned in `rust-toolchain.toml`), egui/eframe (crmne forks), existing `crate::skin`, `crate::vis`, `crate::lyrics`.

**Spec:** `docs/superpowers/specs/2026-10-22-mini-player-client-design.md`. Read it first; this plan argues from it.

## Global Constraints

- Fork only. Upstream's issue-first and no-new-settings rules do not apply. The `AGENTS.md` architecture rules do.
- Views draw and push `Action`s; `App::apply` (`src/app.rs`) mutates. The one established exception, which may be followed: views call `app.ensure_loaded(Page)` (see `src/ui/collection.rs:1658`).
- No network or disk waits on the UI thread. No new crates.
- Everything is drawn in the skin. Frames come from `pledit.bmp` sprites (`src/skin/sprites.rs`), colours from `skin.playlist` (`pledit.txt`), list text from `PixelText` (`src/ui/winamp/pixel_text.rs`), and bar colours from `viscolor.txt` (`VisColors`, `src/skin/config.rs:247`). The only egui UI allowed is the existing popup style, `super::menu(...)` in `src/ui/winamp/mod.rs:769`.
- Queue changes obey `docs/_reference/queue.md`.
- The interface is optimistic. Nothing the user did may flicker away and come back.
- Visualizers show post-EQ, pre-volume audio and keep moving at zero volume.
- Settings stay backward compatible. `Settings` is `#[serde(default)]`, so new fields need `Default` values, and old files must still load.
- All three platforms keep compiling.
- Every task ends green on: `cargo clippy --locked --all-targets --all-features -- -D warnings` and `cargo test --locked --all-targets --all-features`.
- Commit messages end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01Rs2xiJPgDvEdc4zBMsJDN9
  ```
- Work on branch `mini-player-client`, created from `main` before Task 1.

## Review Focus

1. **A search answer that arrives after the user has typed more.** FIND must show results only for the current query, never a stale answer, and must not flicker between the two. Test in Task 3.
2. **Letters typed into FIND triggering shortcuts.** Typing "gas" must not go to the artist, toggle shuffle, or mute. Test in Task 6.
3. **The playing song changing while LYRICS is open and the main window's panel is closed.** The new song's lyrics must load without the user doing anything. Test in Task 7.
4. **A drill-down whose page fails to load** (an offline album, a deleted playlist). FIND shows one error line, BACK still works, and the back stack is not corrupted. Test in Task 4.
5. **Every section open at 4x on a small screen.** The stack height must add up exactly, so the window never clips a section or leaves a gap. Test in Task 1.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/model.rs` (modify) | `WinampSection` enum and the new section `Action`s |
| `src/settings.rs` (modify) | open, shaded and height state for each new section |
| `src/skin/layout.rs` (modify) | size constants for the new sections |
| `src/app.rs` (modify) | applies the new actions; asks for lyrics on track change |
| `src/ui/winamp/mod.rs` (modify) | stacking, toggle buttons, `stack_height` |
| `src/ui/winamp/section.rs` (create) | shared skinned frame: title, close/shade, sides, bottom, grip, scrolling list |
| `src/ui/winamp/playlist.rs` (modify) | `fit` and `rows_for_wheel` move to `section.rs` and are reused from there |
| `src/ui/winamp/find/state.rs` (create) | pure FIND logic: modes, pages, back stack, rows, choosing |
| `src/ui/winamp/find/mod.rs` (create) | draws FIND: input bar, mode row, list, bottom bar |
| `src/ui/winamp/item_menu.rs` (create) | the per-item action menu (right-click and `a p`) |
| `src/ui/winamp/keys.rs` (create) | mini-player keys and the two-key sequence parser |
| `src/ui/winamp/lyrics.rs` (create) | the LYRICS section |
| `src/ui/winamp/vis_section.rs` (create) | the VIS section |
| `src/ui/winamp/devices.rs` (create) | the DEV menu |
| `docs/_guide/winamp.md` (modify) | the user guide, extended as each feature lands |

---

### Task 1: Section state, actions, and stack height

**Files:**
- Modify: `src/model.rs` (near `ToggleWinampEq`, around line 1332), `src/settings.rs:395-430` and its `Default` (around line 544), `src/skin/layout.rs`, `src/app.rs:11757-11865`, `src/ui/winamp/mod.rs:38-74`

**Interfaces:**
- Produces:
  - `pub enum WinampSection { Find, Lyrics, Vis }` in `model.rs`, deriving `Clone, Copy, Debug, PartialEq, Eq, Hash`.
  - `Action::ToggleWinampSection(WinampSection)`, `Action::ToggleWinampSectionShade(WinampSection)`, `Action::SetWinampSectionHeight(WinampSection, u32)`.
  - Settings fields, with defaults: `find_open: bool = false`, `find_shaded: bool = false`, `find_height: u32 = 174`, `lyrics_open: bool = false`, `lyrics_shaded: bool = false`, `lyrics_height: u32 = 116`, `vis_open: bool = false`, `vis_shaded: bool = false`.
  - `Settings::section_open(&self, WinampSection) -> bool`, `section_shaded(&self, WinampSection) -> bool`, and `section_height(&self, WinampSection) -> u32`, which gives the clamped height, or the shade height when shaded.
  - In `layout.rs`: `SECTION_MIN_HEIGHT: u32 = 116`, `SECTION_MAX_HEIGHT: u32 = PLAYLIST_MAX_HEIGHT`, `SECTION_RESIZE_STEP: u32 = 29`, `SECTION_SHADE_HEIGHT: u32 = 14`, `VIS_HEIGHT: u32 = 58`. VIS is never resized.
  - `stack_height` adds FIND, then LYRICS, then VIS after PL, in that order.

- [ ] **Step 1: Write the failing tests**

In `src/ui/winamp/mod.rs` tests:
```rust
#[test]
fn every_open_section_adds_its_height_in_order() {
    let mut s = Settings::default();
    s.find_open = true; s.lyrics_open = true; s.vis_open = true;
    assert_eq!(stack_height(&s), layout::WINDOW_HEIGHT + 174 + 116 + layout::VIS_HEIGHT);
    s.lyrics_shaded = true;
    assert_eq!(stack_height(&s), layout::WINDOW_HEIGHT + 174 + layout::SECTION_SHADE_HEIGHT + layout::VIS_HEIGHT);
    s.eq_open = true; s.playlist_open = true;
    assert_eq!(stack_height(&s),
        layout::WINDOW_HEIGHT + layout::EQ_HEIGHT + s.playlist_height
        + 174 + layout::SECTION_SHADE_HEIGHT + layout::VIS_HEIGHT);
}
```
In `src/settings.rs` tests:
```rust
#[test]
fn settings_from_before_the_sections_still_load() {
    let s: Settings = serde_json::from_str(r#"{"playlist_open":true}"#).unwrap();
    assert!(s.playlist_open && !s.find_open && !s.lyrics_open && !s.vis_open);
    assert_eq!(s.find_height, 174);
}
```
In `src/app.rs` tests, using `test_app`:
```rust
#[test]
fn section_actions_toggle_shade_and_clamp_height() {
    let ctx = egui::Context::default();
    let mut app = test_app("winamp-sections");
    app.apply(Action::ToggleWinampSection(WinampSection::Find), &ctx);
    assert!(app.settings.find_open && app.settings_dirty);
    app.apply(Action::ToggleWinampSectionShade(WinampSection::Lyrics), &ctx);
    assert!(app.settings.lyrics_shaded);
    app.apply(Action::SetWinampSectionHeight(WinampSection::Find, 5), &ctx);
    assert_eq!(app.settings.find_height, layout::SECTION_MIN_HEIGHT);
    app.apply(Action::SetWinampSectionHeight(WinampSection::Vis, 500), &ctx);
    assert_eq!(app.settings.section_height(WinampSection::Vis), layout::VIS_HEIGHT);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features sections`
Expected: compile errors, because `find_open`, `WinampSection` and the other new names don't exist yet.

- [ ] **Step 3: Implement the fields, constants and actions, and extend `stack_height`**

`SetWinampSectionHeight` for `Vis` is a no-op. Match the doc-comment style of the neighbouring fields.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --locked --all-features sections settings_from_before`
Expected: PASS.

- [ ] **Step 5: Commit** with the message `Add state for three new mini player sections`.

---

### Task 2: The skinned section frame, stacking and toggles

**Files:**
- Create: `src/ui/winamp/section.rs`
- Modify: `src/ui/winamp/mod.rs` (`show()` at lines 325-407, `options_menu` at 702, `windows_buttons` at 1428), `src/ui/winamp/playlist.rs` (move `fit` and `rows_for_wheel` out)

**Interfaces:**
- Consumes: Task 1's settings, actions and constants.
- Produces, all in `section.rs`:
  - `pub(super) struct Chrome { pub height: u32 }`
  - `pub(super) fn frame(app: &mut App, view: &mut View, section: WinampSection, title: &str, focused: bool) -> Option<Chrome>`. It draws the shaded bar and returns `None` when the section is shaded. Otherwise it draws the full frame, wires close/shade/title-drag/grip (the grip only for resizable sections), and returns the height.
  - `pub(super) fn list_area(height: u32) -> Area`, the same geometry as the playlist's list area.
  - `pub(super) fn bottom_buttons(view: &mut View, height: u32, labels: &[&str], id: &str) -> Option<usize>`, which returns the index of the button clicked this frame.
  - `pub(super) struct ListRow<'a> { pub label: Cow<'a, str>, pub right: Option<String>, pub current: bool, pub selected: bool }`
  - `pub(super) struct ListResponse { pub clicked: Option<usize>, pub double_clicked: Option<usize>, pub secondary: Option<usize> }`
  - `pub(super) fn list(view: &mut View, text: &mut PixelText, rows: &[ListRow], scroll: &mut usize, wheel: &mut f32, area: Area, id: &str) -> ListResponse`. It draws the rows, the wheel scrolling and the scroll handle.
  - `pub(super) fn fit` and `pub(super) fn rows_for_wheel`, moved unchanged from `playlist.rs`.
  - Per-section scroll and wheel fields on `WinampState` (`src/winamp.rs:69`): `find_scroll`, `find_wheel`, `lyrics_scroll`, `lyrics_wheel`, `section_resize: f32`.

The frame build:
- **Top:** the `PLAYLIST_TOP_*` corners and tiles, with top tiles everywhere the `PLAYLIST_TITLE` sprite would go, so the baked-in "WINAMP PLAYLIST" text never shows. The title is centred, in the skin font, with `view.text`.
- **Sides:** the playlist's side tiles.
- **Bottom:** `PLAYLIST_BOTTOM_TILE` repeated across the full width, so no baked-in ADD/REM buttons show. `bottom_buttons` draws its labels on top in the skin font and gives pressed feedback by drawing the label one pixel lower.
- **Shaded:** the `PLAYLIST_SHADE_*` pieces, with the title text added.

Toggles:
- Draw FIND, LYR and VIS as skin-font text buttons, lit when open, plus DEV (a stub until Task 8). Put them where they look native, and say in the commit message where you put them.
- Add matching checkboxes to the O menu (`options_menu`).
- `show()` draws the open sections after PL, in Task 1's order, each with a `View` at the right `below_y` and `mask: None`. For now each body is empty.

- [ ] **Step 1: Write the failing tests**

In `section.rs`:
```rust
#[test]
fn a_section_list_holds_whole_rows() {
    assert_eq!(list_area(174), Area::new(12, 20, 243, 116));
    assert_eq!(list_area(116).height / layout::PLAYLIST_TRACK_HEIGHT, 4);
}
#[test]
fn bottom_buttons_share_the_bar_without_overlap() {
    let spans = button_spans(&["PLAY", "+QUEUE", "+LIST", "RADIO", "BACK"]);
    assert!(spans.windows(2).all(|w| w[0].x + w[0].width < w[1].x));
    assert!(spans.last().unwrap().x + spans.last().unwrap().width <= layout::WINDOW_WIDTH - layout::PLAYLIST_RIGHT_WIDTH);
}
```
`button_spans(labels: &[&str]) -> Vec<Area>` is a private helper that `bottom_buttons` uses. Each label is 5 px per character, with padding and gaps of 4 px.

Keep the playlist's existing tests passing, now calling `section::rows_for_wheel`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features a_section_list bottom_buttons`
Expected: FAIL to compile, because the module doesn't exist yet.

- [ ] **Step 3: Implement `section.rs`, the `show()` stacking, the toggles and the O-menu items**

- [ ] **Step 4: Run all mini-player tests, then check by eye**

Run: `cargo test --locked --all-features winamp`. Expected: PASS.

Then run `cargo run --locked`, open the mini player (Cmd+Shift+M or Ctrl+M) and check:
- Each section opens, closes, rolls up and resizes. VIS doesn't resize.
- At 2x and with the built-in skin, there are no seams and none of the playlist's baked-in text shows.
- The window height matches the stack.

Take screenshots for the commit description.

- [ ] **Step 5: Commit** with the message `Draw FIND, LYRICS and VIS sections in the skin`.

---

### Task 3: FIND search

**Files:**
- Create: `src/ui/winamp/find/state.rs`, `src/ui/winamp/find/mod.rs`
- Modify: `src/winamp.rs` (add `pub find: crate::ui::winamp::find::FindState` to `WinampState`), `src/ui/winamp/mod.rs` (call `find::show`), and `src/ui/winamp/playlist.rs:644` (ADD → "Search Spotify" now opens and focuses FIND instead of leaving the mini player)

**Interfaces:**
- Consumes: Task 2's `section::{frame, list, bottom_buttons, ListRow}`; `Action::Search(String)`; `app.search: SearchState` (`src/model.rs:730`), whose `results` holds `SearchResults` (`src/api/models.rs:798`) and whose `committed` holds the query the results belong to.
- Produces, in `state.rs`:
  - `pub enum FindMode { Library, Recent, Songs, Artists, Albums, Lists }`
  - `pub enum ItemKind { Song, Artist, Album, Playlist }`
  - `pub struct FindRow { pub uri: String, pub label: String, pub duration_ms: Option<u32>, pub kind: ItemKind, pub album_uri: Option<String>, pub artist_uri: Option<String> }`
  - `pub enum FindPage { Top, Artist { id: String, name: String, albums: bool }, Album { id: String, name: String }, Playlist { id: String, name: String }, Radio { seed: String, name: String } }`
  - `pub struct FindState { pub query: String, pub mode: FindMode, pub stack: Vec<FindPage>, pub selected: Option<usize>, pub focused: bool, pub focus_requested: bool }`. `stack` is never empty, and `stack[0]` is `Top`.
  - `pub const RESULT_LIMIT: usize = 15`
  - `impl FindState`:
    - `pub fn set_query(&mut self, query: String) -> Vec<Action>`. It updates `mode`: an empty query switches to `Library`, unless the mode is already `Library` or `Recent`; a non-empty query switches to `Songs`, unless it is already a search mode. It also pops the stack to `Top`, and returns `[Action::Search(query)]` when the query is non-empty.
    - `pub fn modes(&self) -> &'static [FindMode]`: `[Library, Recent]` when the query is empty, otherwise `[Songs, Artists, Albums, Lists]`.
    - `pub fn cycle_mode(&mut self, forward: bool)`
    - `pub fn top_search_rows(&self, results: &SearchResults) -> Vec<FindRow>`: rows of the current mode's kind, capped at `RESULT_LIMIT`.
    - `pub fn title(&self) -> String`: `"FIND"`, or `"FIND · <name>"` while inside a page.
  - In `mod.rs`: `pub(super) fn show(app: &mut App, view: &mut View, focused: bool)`.

The input bar:
- Draw it in the list colours with `PixelText` and a blinking caret.
- While FIND has focus, take `egui::Event::Text`, Backspace, Cmd/Ctrl+Backspace (clear the box) and paste from `ctx.input`.
- An X at its right clears the box.
- Searches are debounced at 300 ms with `SearchState.typed_at`, matching the main search.
- Show results only when `app.search.committed == state.query`. Otherwise keep the last rows that matched, which is the stale-answer rule.

The bottom bar is PLAY, +QUEUE, +LIST, RADIO, BACK. In this task:
- PLAY and double-click/Enter on a song push `Action::PlayUris { uris: vec![uri], index: 0 }`.
- +QUEUE pushes `Action::AddToQueue { uri, label }`.
- +LIST and RADIO stay inert until Tasks 4 and 5.
- On an artist, album or playlist row, choosing does nothing until Task 4.

- [ ] **Step 1: Write the failing tests**

In `state.rs`:
```rust
#[test]
fn typing_switches_to_search_modes_and_clearing_returns_to_library() {
    let mut s = FindState::default();
    assert_eq!(s.mode, FindMode::Library);
    assert!(matches!(s.set_query("radiohead".into()).as_slice(), [Action::Search(q)] if q == "radiohead"));
    assert_eq!(s.mode, FindMode::Songs);
    s.mode = FindMode::Albums;
    s.set_query("radioheads".into());
    assert_eq!(s.mode, FindMode::Albums);
    assert!(s.set_query(String::new()).is_empty());
    assert_eq!(s.mode, FindMode::Library);
    assert_eq!(s.modes(), &[FindMode::Library, FindMode::Recent]);
}
#[test]
fn search_rows_are_the_modes_kind_and_at_most_fifteen() {
    let results = search_results(16, 3); // test helper: 16 tracks, 3 artists, built in the test module
    let mut s = FindState::default(); s.set_query("x".into());
    let rows = s.top_search_rows(&results);
    assert!(rows.len() <= RESULT_LIMIT && rows.iter().all(|r| r.kind == ItemKind::Song));
    s.mode = FindMode::Artists;
    assert!(s.top_search_rows(&results).iter().all(|r| r.kind == ItemKind::Artist));
}
```
`search_results(tracks, artists)` builds a `SearchResults` from JSON, in the same shape as the test at `src/api/models.rs:1126`.

In `find/mod.rs`, with `test_app` and `crate::demo::populate`:
```rust
#[test]
fn an_answer_for_an_older_query_is_not_shown() {
    // state.query = "radiohead"; app.search.committed = "radio"; results loaded
    // → visible_rows(&app) is empty (or the previously shown rows), never the "radio" rows.
}
```
`fn visible_rows(app: &App) -> Vec<FindRow>` is the function `show` draws from. Expose it as `pub(super)` so it can be tested.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features find::`
Expected: compile errors, because the module doesn't exist yet.

- [ ] **Step 3: Implement `state.rs`, then `mod.rs`**

- [ ] **Step 4: Run the tests, then check by eye**

Run: `cargo test --locked --all-features find::`. Expected: PASS.

Then in the running app, check:
- Typing, switching modes and scrolling all work.
- Double-clicking a song plays it.
- +QUEUE adds it, and it shows up in PL straight away.

- [ ] **Step 5: Commit** with the message `Search Spotify from the mini player's FIND section`.

---

### Task 4: Library, recent, and drill-down

**Files:**
- Modify: `src/ui/winamp/find/state.rs`, `src/ui/winamp/find/mod.rs`

**Interfaces:**
- Consumes:
  - Library data: `app.library.liked`, `.albums`, `.artists`, `.playlists` (`src/model.rs:566`).
  - Recent: `app.home.recently_played`.
  - Pages: `app.album_pages`, `app.artist_pages` (`top_tracks`, `albums`), `app.playlist_pages`, `app.radio_pages`.
  - Loaders: `app.ensure_loaded(Page::LikedSongs | Page::Albums | Page::Artists | Page::Home | Page::Album(id) | Page::Artist(id) | Page::Playlist(id) | Page::Radio(seed))`, and `Action::LoadMore(Page)` near the end of a list.
- Produces, in `state.rs`:
  - `pub enum Choice { Play(Action), Open(FindPage) }`
  - `pub fn choose(&self, row: &FindRow, rows: &[FindRow], index: usize) -> Choice`. Its behaviour:
    - A song inside an album, playlist, artist's top songs or radio gives `Play(Action::PlayContext { uri: <context uri>, offset_uri: Some(row.uri), offset_index: None })`. For the artist's top songs and radio, which have no context URI, it gives `Play(Action::PlayUris { uris: <all song rows>, index })` instead.
    - A song from `Top` (search, recent, Liked Songs) gives `PlayUris { uris: vec![uri], index: 0 }`. A Liked Songs row plays in the Liked Songs context, the same way the main window's Liked Songs page does. Copy what that page pushes.
    - An artist, album or playlist row gives `Open(FindPage::…)`.
  - `pub fn open(&mut self, page: FindPage)`: pushes the page and resets `selected`.
  - `pub fn back(&mut self) -> bool`: pops the stack. It returns false at `Top`.
  - `pub fn page(&self) -> &FindPage`
  - `pub(super) fn page_rows(app: &App, page: &FindPage) -> Loadable<Vec<FindRow>>`, in `mod.rs`.

Drawing:
- The artist page shows a second mode row, SONGS / ALBUMS, which toggles `FindPage::Artist { albums }`.
- BACK and Backspace (with the box empty) call `back()`.
- A page whose load failed shows one error line in the list's "current" colour. Clicking it calls `ensure_loaded` again. BACK still works.
- Every library list caps at nothing and loads more as you scroll; search results stay capped at 15.

- [ ] **Step 1: Write the failing tests**

In `state.rs`:
```rust
#[test]
fn choosing_an_album_opens_it_and_back_returns() {
    let mut s = FindState::default();
    let album = FindRow { uri: "spotify:album:a".into(), label: "In Rainbows".into(), kind: ItemKind::Album, ..row() };
    assert!(matches!(s.choose(&album, &[album.clone()], 0), Choice::Open(FindPage::Album { id, .. }) if id == "a"));
    s.open(FindPage::Album { id: "a".into(), name: "In Rainbows".into() });
    assert_eq!(s.title(), "FIND · In Rainbows");
    assert!(s.back()); assert!(!s.back());
    assert_eq!(s.page(), &FindPage::Top);
}
#[test]
fn a_song_inside_an_album_plays_in_the_album() {
    let mut s = FindState::default();
    s.open(FindPage::Album { id: "a".into(), name: "A".into() });
    let song = FindRow { uri: "spotify:track:t".into(), kind: ItemKind::Song, ..row() };
    assert!(matches!(s.choose(&song, &[song.clone()], 0),
        Choice::Play(Action::PlayContext { uri, offset_uri: Some(o), .. }) if uri == "spotify:album:a" && o == "spotify:track:t"));
}
#[test]
fn a_searched_song_plays_on_its_own() { /* Top + Song → PlayUris { uris: [uri], index: 0 } */ }
```
In `mod.rs`:
```rust
#[test]
fn a_page_that_failed_to_load_shows_one_line_and_keeps_back() {
    // app.album_pages["a"].album = Loadable::Failed(..); state opened on Album "a"
    // → page_rows(..) is Failed; after back(), page() == Top and rows are the library rows.
}
```
`row()` is a test helper that returns a default `FindRow`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features find::`
Expected: FAIL, because `choose`, `open` and `back` don't exist yet.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the tests, then check by eye**

Run: `cargo test --locked --all-features find::`. Expected: PASS.

Then check by hand, at 1x and 2x:
- LIBRARY and RECENT show your items.
- Open an artist, then one of their albums, then go back twice.

- [ ] **Step 5: Commit** with the message `Browse library, recent, artists and albums inside FIND`.

---

### Task 5: Item actions menu

**Files:**
- Create: `src/ui/winamp/item_menu.rs`
- Modify: `src/ui/winamp/find/mod.rs` (right-click on a row, and the +LIST and RADIO buttons), `src/ui/winamp/playlist.rs` (right-click on a row)

**Interfaces:**
- Consumes: `FindRow` and `ItemKind` from Task 3.
- Uses these existing actions:
  - queue and playing: `PlayNext { items }`, `AddToQueue`
  - saving: `ToggleSaved(uri)`, which also follows/unfollows artists and playlists and saves albums, as `set_saved` does
  - playlists: `RemoveFromPlaylist`, and `add_to_playlist_menu` (`src/ui/widgets.rs:772`), which must become `pub(crate)`
  - other: `CopyLink(uri)`
- Produces:
  - `pub(super) enum ItemAction { Play, PlayNext, AddToQueue, AddToPlaylist, ToggleSaved, RemoveFromPlaylist, GoToArtist, GoToAlbum, Radio, CopyLink }`
  - `pub(super) fn actions_for(kind: ItemKind, in_own_playlist: bool) -> &'static [ItemAction]`. It follows the spec's lists: song, artist, and album/playlist.
  - `pub(super) fn menu(app: &mut App, ui: &mut egui::Ui, row: &FindRow, context: Option<&RowContext>)`, which draws the menu with `super::menu`.
  - `pub(super) fn perform(app: &mut App, row: &FindRow, action: ItemAction, context: Option<&RowContext>)`. It pushes the matching `Action`s. GoToArtist, GoToAlbum and Radio open FIND if it's closed and push the right `FindPage` (`Radio { seed: row.uri }`). They never touch the main window.
  - Like/unlike shows "Unlike", "Unfollow" or "Remove from library" when `app.is_saved(uri) == Some(true)`.

- [ ] **Step 1: Write the failing tests**

In `item_menu.rs`:
```rust
#[test]
fn each_kind_offers_the_spec_actions() {
    use ItemAction::*;
    assert_eq!(actions_for(ItemKind::Song, false),
        &[Play, PlayNext, AddToQueue, AddToPlaylist, ToggleSaved, GoToArtist, GoToAlbum, Radio, CopyLink]);
    assert!(actions_for(ItemKind::Song, true).contains(&RemoveFromPlaylist));
    assert_eq!(actions_for(ItemKind::Artist, false), &[Play, ToggleSaved, Radio, CopyLink]);
    assert_eq!(actions_for(ItemKind::Album, false), &[Play, AddToQueue, ToggleSaved, Radio, CopyLink]);
}
#[test]
fn go_to_album_opens_it_inside_find_not_the_main_window() {
    // test_app; perform(GoToAlbum) on a song row with album_uri "spotify:album:a"
    // → app.settings.find_open (via ToggleWinampSection action applied) and
    //   app.winamp.find.page() == Album{id:"a"}; no Action::Open(_) pushed.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features item_menu`
Expected: FAIL to compile, because the module doesn't exist yet.

- [ ] **Step 3: Implement it, and wire right-click in FIND and PL, +LIST (opens the playlist submenu) and RADIO**

- [ ] **Step 4: Run the tests, then check by eye**

Run: `cargo test --locked --all-features item_menu`. Expected: PASS.

Then check by hand:
- A song added to a playlist, a liked song and a followed artist all show their new state straight away.
- Copy link puts a `https://open.spotify.com/...` link on the clipboard.

- [ ] **Step 5: Commit** with the message `Act on any song, artist, album or playlist from the mini player`.

---

### Task 6: Keyboard

**Files:**
- Create: `src/ui/winamp/keys.rs`
- Modify:
  - `src/ui/winamp/mod.rs:331`: call `keys::handle(app, &ctx)` before `super::keys::handle`, so that the mini-player keys win.
  - `src/ui/keys.rs` (`shortcuts()` at line 192): list the mini-player keys under a "Mini player" heading.

**Interfaces:**
- Consumes: `FindState` (focus, `cycle_mode`, `back`, `selected`), `item_menu::perform`, `WinampSection` toggles.
- Produces:
  - `pub(super) enum Verb { Go, Add, Remove }`
  - `pub(super) struct Sequencer { pending: Option<(Verb, Instant)> }`, with `pub(super) const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(1)`.
  - `impl Sequencer { pub(super) fn press(&mut self, key: char, now: Instant) -> Option<Command> }`
  - `pub(super) enum Command { Section(WinampSection), Eq, Playlist, FocusFind, Item(ItemAction), Recent, Library, Devices, ShowPlaying, RemoveFromQueue }`
  - `pub(super) fn pending_label(&self) -> Option<&'static str>`: `"g…"`, `"a…"` or `"d…"`, for the marquee to show.

The mapping, copied from the spec:

| Keys | Command |
|---|---|
| `/` | `FocusFind` |
| `e` | `Eq` |
| `q` | `Playlist` |
| `l` | `Section(Lyrics)` |
| `v` | `Section(Vis)` |
| `y` | `Item(CopyLink)` |
| `g a` | `Item(GoToArtist)` |
| `g b` | `Item(GoToAlbum)` |
| `g r` | `Item(Radio)` |
| `g h` | `Recent` |
| `g y` | `Library` |
| `g d` | `Devices` |
| `g p` | `ShowPlaying` |
| `a q` | `Item(AddToQueue)` |
| `a n` | `Item(PlayNext)` |
| `a p` | `Item(AddToPlaylist)` |
| `a l` | `Item(ToggleSaved)`, only when not saved |
| `d l` | `Item(ToggleSaved)`, only when saved |
| `d p` | `Item(RemoveFromPlaylist)` |
| `d q` | `RemoveFromQueue` |

Rules:
- An unknown second key cancels the sequence and does nothing else.
- Cmd/Ctrl+F also gives `FocusFind`.
- While FIND's box is focused, `handle` consumes nothing except ↑, ↓, Enter, Esc, Tab, Shift+Tab and Backspace (on an empty box).
- Esc clears the box, and a second Esc closes FIND.
- Item commands act on the selected row in whichever of FIND and PL was clicked last, falling back to the playing song.
- Space, `m`, `s`, `r`, `b` and `?` fall through to the global handler unchanged.

- [ ] **Step 1: Write the failing tests**

In `keys.rs`:
```rust
#[test]
fn verbs_wait_for_a_second_key() {
    let t = Instant::now(); let mut s = Sequencer::default();
    assert_eq!(s.press('g', t), None);
    assert_eq!(s.pending_label(), Some("g…"));
    assert_eq!(s.press('a', t), Some(Command::Item(ItemAction::GoToArtist)));
    assert_eq!(s.press('a', t), None);
    assert_eq!(s.press('q', t), Some(Command::Item(ItemAction::AddToQueue)));
}
#[test]
fn a_sequence_expires_after_a_second_and_unknown_keys_cancel() {
    let t = Instant::now(); let mut s = Sequencer::default();
    s.press('g', t);
    assert_eq!(s.press('a', t + SEQUENCE_TIMEOUT + Duration::from_millis(1)), None); // 'a' now starts Add
    assert_eq!(s.pending_label(), Some("a…"));
    assert_eq!(s.press('z', t), None);
    assert_eq!(s.pending_label(), None);
}
#[test]
fn single_keys_map_to_sections() {
    let t = Instant::now(); let mut s = Sequencer::default();
    assert_eq!(s.press('l', t), Some(Command::Section(WinampSection::Lyrics)));
    assert_eq!(s.press('/', t), Some(Command::FocusFind));
}
#[test]
fn typing_in_find_never_triggers_shortcuts() {
    // test_app, FIND focused; feed Text("g"), Text("a"), Text("s"), Text("m") through handle()
    // → app.winamp.find.query == "gasm"; no Action pushed besides Search; shuffle and mute unchanged.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features winamp::keys`
Expected: FAIL to compile, because the module doesn't exist yet.

- [ ] **Step 3: Implement `Sequencer` and `handle`, and add the marquee hint and the shortcuts listing**

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --locked --all-features winamp::keys keys::`
Expected: PASS, and the existing keys tests in `src/ui/keys.rs` still pass.

- [ ] **Step 5: Commit** with the message `Drive the mini player from the keyboard`.

---

### Task 7: LYRICS

**Files:**
- Create: `src/ui/winamp/lyrics.rs`
- Modify: `src/app.rs` (call `request_lyrics` on track change while `settings.lyrics_open`, and when opening the section), `src/ui/winamp/mod.rs`

**Interfaces:**
- Consumes:
  - lyrics data: `app.lyrics: Loadable<Option<Lyrics>>`, `Lyrics::active_line(position_ms)` (`src/lyrics.rs:56`), `Line { at_ms, text }`
  - following state: `app.lyrics_following`
  - actions: `FollowLyrics`, `PauseLyricsFollow`, `RetryLyrics`, `Seek(ms)`
- Produces:
  - `pub(super) fn show(app: &mut App, view: &mut View, now: Option<&NowPlaying>, focused: bool)`
  - `pub(super) fn first_visible(active: Option<usize>, visible: usize, len: usize) -> usize`: centres the active line, clamped to the ends.

Drawing:
- The active line uses the skin's `current` colour and the other lines its `normal` colour.
- Lines are centred, cut to fit with `section::fit`, one per row.
- A scroll with the wheel pushes `PauseLyricsFollow`, and the list then scrolls freely. A track change, or a click on the active line, pushes `FollowLyrics`.
- A click on a synced line pushes `Seek(at_ms)`.
- Single-line states:
  - Loading: `"…"`
  - No lyrics: `"No lyrics"`
  - Instrumental: `"Instrumental"`
  - Failed: the error text, and a click pushes `RetryLyrics`
- All of these strings go through `gettext`.

Hook: find where the playing URI changes in `app.rs`, at the code that already resets `lyrics_uri`. At that point, call `self.request_lyrics()` if `self.settings.lyrics_open`. `ToggleWinampSection(Lyrics)` also calls it when it opens the section.

- [ ] **Step 1: Write the failing tests**

In `lyrics.rs`:
```rust
#[test]
fn the_active_line_sits_in_the_middle_until_the_ends() {
    assert_eq!(first_visible(Some(0), 5, 40), 0);
    assert_eq!(first_visible(Some(10), 5, 40), 8);
    assert_eq!(first_visible(Some(39), 5, 40), 35);
    assert_eq!(first_visible(None, 5, 3), 0);
}
```
In `app.rs`:
```rust
#[test]
fn an_open_lyrics_section_asks_for_the_new_songs_lyrics() {
    // test_app + demo::populate; settings.lyrics_open = true; show_lyrics_panel = false;
    // simulate the playing track changing to a new URI (use the same event path the
    // existing lyrics tests use) → app.lyrics_uri == Some(new) and app.lyrics is Loading.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --all-features lyrics`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the tests, then check by eye**

Run: `cargo test --locked --all-features lyrics`. Expected: PASS.

Then, with a synced song playing, check that LYRICS follows the song, that clicking a line jumps the song there, and that scrolling pauses following.

- [ ] **Step 5: Commit** with the message `Show lyrics in the mini player`.

---

### Task 8: Devices menu

**Files:**
- Create: `src/ui/winamp/devices.rs`
- Modify: `src/ui/winamp/mod.rs` (the DEV button, and `g d` opening it)

**Interfaces:**
- Consumes: `app.devices: Vec<Device>` (`id`, `name`, `is_active`, `is_restricted`), `app.devices_loading`, `Action::RefreshDevices`, `Action::Transfer(String)`. Find out how `src/ui/player_bar.rs` lists this computer and devices found on the local network, and list the same entries in the same order.
- Produces:
  - `pub(super) fn menu(app: &mut App, ui: &mut egui::Ui)`
  - `WinampState.open_devices: bool`, which `g d` sets so the menu opens on the next frame. This is the same pattern as `open_presets`.

Behaviour:
- Opening the menu pushes `RefreshDevices`.
- The active device has a check mark, and restricted devices are greyed out.
- Choosing a device pushes `Transfer(id)`, and the marquee shows `"Playing on <name>"` once.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_active_device_is_checked_and_choosing_transfers() {
    // test_app; app.devices = [this computer (active), "Kitchen" (id "k")];
    // devices::entries(&app) → [("This computer", true), ("Kitchen", false)];
    // devices::choose(&mut app, "k") pushes Action::Transfer("k").
}
```
`entries` and `choose` are `pub(super)` helpers that `menu` uses.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --locked --all-features devices`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the test, then check by eye**

Run: `cargo test --locked --all-features devices`. Expected: PASS.

Then switch playback to a phone or speaker and back, and check that the mini player follows.

- [ ] **Step 5: Commit** with the message `Pick the playback device from the mini player`.

---

### Task 9: VIS

**Files:**
- Create: `src/ui/winamp/vis_section.rs`
- Modify: `src/winamp.rs` (add a `pub wide: crate::vis::WideAnalyser` to `WinampState`, separate from the player bar's), `src/ui/winamp/mod.rs` (the visualiser frame request must include VIS)

**Interfaces:**
- Consumes:
  - the analyser: `vis::WideAnalyser::step(&samples, now) -> [f32; WIDE_BANDS]`, `peaks()` and `settled()` (`src/vis.rs:554-640`)
  - the audio: `app.winamp.tap.window(..)`, called the same way the existing analyser is called in `mod.rs::visualiser`
  - colours: `skin.vis_colors: VisColors`
- Produces:
  - `pub(super) fn show(app: &mut App, view: &mut View, now: Option<&NowPlaying>, focused: bool) -> bool`, which returns whether it is still moving.
  - `pub(super) fn bar_color(colors: &VisColors, height: f32, row: u32, rows: u32) -> Color32`.

Drawing:
- The graph area sits inside the section frame, at `VIS_HEIGHT` minus the frame.
- It draws 75 one-pixel bars with one-pixel gaps, scaled to fit the 243 px list width (round down, then centre).
- Colours run from the bottom of a bar to its top, like Winamp's analyser. Spotifast's analyser already maps rows to `vis_colors` indices 17 down to 2; reuse that mapping. Peak caps use colour index 23.
- When playback is on another device, it draws a flat line at the bottom and centres `"Playing on <device>"` in the list's normal colour.
- It reads the tap, so it is post-EQ and pre-volume, as the rule requires.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn bars_take_the_skins_analyser_colours_bottom_to_top() {
    let c = crate::skin::config::DEFAULT_VIS_COLORS;
    assert_eq!(bar_color(&c, 1.0, 0, 16), rgb(c[17]));
    assert_eq!(bar_color(&c, 1.0, 15, 16), rgb(c[2]));
}
```
`rgb` is a test helper.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --locked --all-features vis_section`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the test, then check by eye**

Run: `cargo test --locked --all-features vis_section`. Expected: PASS.

Then check by hand:
- At zero volume the bars still move.
- With the EQ bass at -12 dB the left bars drop.
- On another device it shows the message.
- With a non-default skin, the bars use that skin's colours.

- [ ] **Step 5: Commit** with the message `Add a wide spectrum visualizer section`.

---

### Task 10: Guide, and the full check

**Files:**
- Modify: `docs/_guide/winamp.md`

- [ ] **Step 1: Update the guide**

Add a section each for FIND, LYRICS, VIS and Devices, plus the key table from Task 6. Remove the sentence saying ADD sends you to the main window. Keep the guide's existing plain style.

- [ ] **Step 2: Run the full check**

Run:
```
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo test --locked --all-features --doc
RUSTDOCFLAGS='-D warnings' cargo doc --locked --all-features --no-deps
```
Expected: all pass with no warnings.

- [ ] **Step 3: Do the success-criterion run**

For about an hour, with the main window never opened:
1. Search, and play a song.
2. Open an artist, then an album, and go back.
3. Queue a song, and add one to a playlist.
4. Like a song.
5. Play from recently played.
6. Switch device and back.
7. Read the lyrics.
8. Use the keyboard only for the steps above.

Note anything that forced a trip to the main window and report it.

- [ ] **Step 4: Commit** with the message `Document the mini player's new sections`.
