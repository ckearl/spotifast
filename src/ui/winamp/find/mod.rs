//! FIND: search Spotify from the mini player.
//!
//! From the top: a box to type in, drawn in the list's colours with the
//! playlist's font and a blinking caret; the row of modes; the list; and
//! the bottom bar's buttons. The section takes the keyboard while it has
//! focus, which a click in it gives and a click elsewhere in the window,
//! or Escape, takes away.

use std::borrow::Cow;
use std::time::{Duration, Instant};

use egui::{Color32, Event, Key, Sense};

use crate::api::models::{PlayableItem, Track};
use crate::app::App;
use crate::i18n::gettext;
use crate::model::{Action, Loadable, Page, WinampSection};
use crate::skin::layout::{self, Area};
use crate::util;

use super::section::{self, ListRow};
use super::{PixelText, View};

pub mod state;

pub use state::FindState;
use state::{Choice, FindMode, FindPage, FindRow, ItemKind};

/// One row of text, as the playlist's.
const ROW: u32 = layout::PLAYLIST_TRACK_HEIGHT;
/// The bottom bar. +LIST and RADIO arrive with the actions.
const BUTTONS: [&str; 5] = ["PLAY", "+QUEUE", "+LIST", "RADIO", "BACK"];
const PLAY: usize = 0;
const QUEUE: usize = 1;
const BACK: usize = 4;
/// Half a blink of the caret.
const BLINK: Duration = Duration::from_millis(530);

/// Which halves of the answer for what the box holds have arrived: the
/// catalogue (songs, artists, albums), and the playlists. A split search
/// sends them apart, and either can land first.
fn arrived(app: &App) -> (bool, bool) {
    let search = &app.search;
    let query = app.winamp.find.query.trim();
    let Loadable::Loaded(results) = &search.results else {
        return (false, false);
    };
    if query.is_empty() || search.committed != query || search.results_serial != search.serial {
        return (false, false);
    }
    let catalogue = !search.catalogue_pending;
    // Unsplit, the playlists come inside the catalogue.
    let lists = !search.playlists_pending && (catalogue || results.playlists.is_some());
    (catalogue, lists)
}

/// The search results the list shows. The answer is shown only for what
/// the box holds, and only once the mode's own half of it has come; until
/// then the last answer shown stays, so the rows never belong to a query
/// the user has typed past, nor flicker away.
fn search_rows(app: &App) -> Vec<FindRow> {
    let find = &app.winamp.find;
    let (catalogue, lists) = arrived(app);
    let live = if find.mode == FindMode::Lists {
        lists
    } else {
        catalogue
    };
    match &app.search.results {
        Loadable::Loaded(results) if live => find.top_search_rows(results),
        _ => find.top_search_rows(&find.held.results),
    }
}

/// A page's rows as far as they have come.
#[derive(Default)]
struct Listing {
    rows: Vec<FindRow>,
    /// Why the rows stop short, and the page that loads them again.
    failed: Option<(String, Page)>,
    /// More rows are on their way.
    loading: bool,
    /// What asks for the next rows, once the list is scrolled near its end.
    more: Option<Action>,
}

impl Listing {
    fn fail(&mut self, error: &str, page: Page) {
        self.failed = Some((error.to_string(), page));
    }
}

/// A song's row: its artists and name, its length, and where it is from.
fn song_row(track: &Track) -> FindRow {
    let artists = track.artist_names();
    let label = if artists.is_empty() {
        track.name.clone()
    } else {
        format!("{artists} - {}", track.name)
    };
    let present = |uri: Option<&String>| uri.filter(|uri| !uri.is_empty()).cloned();
    FindRow {
        uri: track.uri.clone(),
        label,
        name: track.name.clone(),
        duration_ms: Some(track.duration_ms),
        kind: ItemKind::Song,
        album_uri: present(track.album.as_ref().map(|album| &album.uri)),
        artist_uri: present(track.artists.first().and_then(|artist| artist.uri.as_ref())),
        context: None,
    }
}

/// An artist, album or playlist's row.
fn item_row(uri: &str, label: String, name: &str, kind: ItemKind) -> FindRow {
    FindRow {
        uri: uri.to_string(),
        label,
        name: name.to_string(),
        kind,
        ..FindRow::default()
    }
}

/// The library: your playlists, saved albums, followed artists, then Liked
/// Songs. Each list follows only once the one above it is whole, so rows
/// arrive at the end and never push down the ones in view; the longest,
/// Liked Songs, comes last so the others are a short scroll away.
fn library(app: &App) -> Listing {
    let library = &app.library;
    let mut listing = Listing::default();
    if let Loadable::Loaded(playlists) = &library.playlists {
        listing.rows.extend(
            playlists
                .iter()
                .map(|list| item_row(&list.uri, list.name.clone(), &list.name, ItemKind::Playlist)),
        );
    }

    let albums = &library.albums;
    listing.rows.extend(albums.items.iter().map(|saved| {
        let album = &saved.album;
        let artists =
            crate::api::models::join_names(album.artists.iter().map(|artist| artist.name.as_str()));
        let label = if artists.is_empty() {
            album.name.clone()
        } else {
            format!("{artists} - {}", album.name)
        };
        item_row(&album.uri, label, &album.name, ItemKind::Album)
    }));
    if let Some(error) = &albums.error {
        listing.fail(error, Page::Albums);
        return listing;
    }
    if !albums.is_complete() {
        listing.loading = true;
        listing.more = albums
            .can_load_more()
            .then_some(Action::LoadMore(Page::Albums));
        return listing;
    }

    let artists = &library.artists;
    listing.rows.extend(artists.items.iter().map(|artist| {
        item_row(
            &artist.uri,
            artist.name.clone(),
            &artist.name,
            ItemKind::Artist,
        )
    }));
    if let Some(error) = &artists.error {
        listing.fail(error, Page::Artists);
        return listing;
    }
    if !artists.complete {
        listing.loading = true;
        listing.more = artists
            .can_load_more()
            .then_some(Action::LoadMore(Page::Artists));
        return listing;
    }

    // Played as the big window's Liked Songs page plays them: in the
    // collection once it is whole.
    let liked = &library.liked;
    let collection = app
        .user
        .as_ref()
        .filter(|_| liked.is_complete())
        .map(|user| format!("spotify:user:{}:collection", user.id));
    listing.rows.extend(liked.items.iter().map(|saved| FindRow {
        context: collection.clone(),
        ..song_row(&saved.track)
    }));
    if let Some(error) = &liked.error {
        listing.fail(error, Page::LikedSongs);
    } else if !liked.is_complete() {
        listing.loading = true;
        listing.more = liked
            .can_load_more()
            .then_some(Action::LoadMore(Page::LikedSongs));
    }
    listing
}

/// A list Spotify answers whole: a radio's songs, an artist's top songs,
/// the recently played.
fn whole<T>(loadable: &Loadable<Vec<T>>, row: impl Fn(&T) -> FindRow, retry: Page) -> Listing {
    let mut listing = Listing::default();
    match loadable {
        Loadable::Loaded(items) => listing.rows = items.iter().map(row).collect(),
        Loadable::Failed(error) => listing.fail(error, retry),
        Loadable::Loading | Loadable::NotLoaded => listing.loading = true,
    }
    listing
}

/// What `page` lists now.
fn listing(app: &App, page: &FindPage) -> Listing {
    match page {
        FindPage::Top => match app.winamp.find.mode {
            FindMode::Library => library(app),
            FindMode::Recent => whole(
                &app.home.recently_played,
                |played| song_row(&played.track),
                Page::Home,
            ),
            _ => Listing {
                rows: search_rows(app),
                ..Listing::default()
            },
        },
        FindPage::Album { id, .. } => {
            let mut listing = Listing::default();
            let Some(album) = app.album_pages.get(id) else {
                listing.loading = true;
                return listing;
            };
            let uri = format!("spotify:album:{id}");
            listing.rows = album
                .tracks
                .items
                .iter()
                .map(|track| FindRow {
                    album_uri: Some(uri.clone()),
                    ..song_row(track)
                })
                .collect();
            let tracks = &album.tracks;
            let retry = Page::Album(id.clone());
            if let Loadable::Failed(error) = &album.album {
                // The album itself did not come: the page failed.
                listing.rows.clear();
                listing.fail(error, retry);
            } else if let Some(error) = &tracks.error {
                listing.fail(error, retry);
            } else {
                listing.loading = album.album.is_loading() || tracks.loading || !tracks.loaded_once;
                if tracks.loaded_once && tracks.can_load_more() {
                    listing.more = Some(Action::LoadMore(retry));
                }
            }
            listing
        }
        FindPage::Playlist { id, .. } => {
            let mut listing = Listing::default();
            let Some(playlist) = app.playlist_pages.get(id) else {
                listing.loading = true;
                return listing;
            };
            let items = &playlist.items;
            listing.rows = items
                .items
                .iter()
                .filter_map(|item| match item.playable()? {
                    PlayableItem::Track(track) => Some(song_row(track)),
                    PlayableItem::Episode(episode) => {
                        let show = episode.show.as_ref().map(|show| show.name.as_str());
                        Some(FindRow {
                            uri: episode.uri.clone(),
                            label: match show {
                                Some(show) if !show.is_empty() => {
                                    format!("{show} - {}", episode.name)
                                }
                                _ => episode.name.clone(),
                            },
                            name: episode.name.clone(),
                            duration_ms: Some(episode.duration_ms),
                            ..FindRow::default()
                        })
                    }
                })
                .collect();
            let retry = Page::Playlist(id.clone());
            if let Loadable::Failed(error) = &playlist.playlist {
                listing.rows.clear();
                listing.fail(error, retry);
            } else if let Some(error) = &items.error {
                listing.fail(error, retry);
            } else {
                listing.loading = items.loading || !items.loaded_once;
                if items.loaded_once && items.next_offset.is_some() && items.can_load_more() {
                    listing.more = Some(Action::LoadMore(retry));
                }
            }
            listing
        }
        FindPage::Artist { id, albums, .. } => {
            let retry = Page::Artist(id.clone());
            let Some(artist) = app.artist_pages.get(id) else {
                return Listing {
                    loading: true,
                    ..Listing::default()
                };
            };
            if let Loadable::Failed(error) = &artist.artist {
                let mut listing = Listing::default();
                listing.fail(error, retry);
                return listing;
            }
            if !albums {
                return whole(&artist.top_tracks, song_row, retry);
            }
            let mut listing = Listing::default();
            let Some(list) = artist.albums.get(artist.filter.groups()) else {
                listing.loading = true;
                return listing;
            };
            // One row per title, as the big window's discography shows.
            let mut seen = std::collections::HashSet::new();
            listing.rows = list
                .items
                .iter()
                .filter(|album| seen.insert(album.name.to_lowercase()))
                .map(|album| item_row(&album.uri, album.name.clone(), &album.name, ItemKind::Album))
                .collect();
            if let Some(error) = &list.error {
                listing.fail(error, retry);
            } else {
                listing.loading = list.loading || !list.loaded_once;
                if list.loaded_once && list.can_load_more() {
                    listing.more = Some(Action::LoadMoreArtistAlbums(id.clone()));
                }
            }
            listing
        }
        FindPage::Radio { seed, .. } => match app.radio_pages.get(seed) {
            Some(radio) => whole(&radio.songs, song_row, Page::Radio(seed.clone())),
            None => Listing {
                loading: true,
                ..Listing::default()
            },
        },
    }
}

/// A page's rows: loading or failed while it has none to show.
pub(super) fn page_rows(app: &App, page: &FindPage) -> Loadable<Vec<FindRow>> {
    let listing = listing(app, page);
    match listing.failed {
        Some((error, _)) if listing.rows.is_empty() => Loadable::Failed(error),
        _ if listing.rows.is_empty() && listing.loading => Loadable::Loading,
        _ => Loadable::Loaded(listing.rows),
    }
}

/// The rows the list shows: none while its page loads or has failed.
pub(super) fn visible_rows(app: &App) -> Vec<FindRow> {
    match page_rows(app, app.winamp.find.page()) {
        Loadable::Loaded(rows) => rows,
        _ => Vec::new(),
    }
}

/// Asks for what the page shown lists, once, when nothing of it is held:
/// as the big window's pages do on opening.
fn fetch(app: &mut App) {
    let page = app.winamp.find.page().clone();
    match page {
        FindPage::Top => match app.winamp.find.mode {
            FindMode::Library => {
                let liked = &app.library.liked;
                if !liked.loaded_once && !liked.loading && liked.error.is_none() {
                    app.ensure_loaded(Page::LikedSongs);
                }
                // Each loads only its first page, once.
                app.ensure_loaded(Page::Albums);
                app.ensure_loaded(Page::Artists);
            }
            FindMode::Recent => {
                if matches!(app.home.recently_played, Loadable::NotLoaded) {
                    app.ensure_loaded(Page::Home);
                }
            }
            _ => {}
        },
        FindPage::Album { id, .. } => {
            if !app.album_pages.contains_key(&id) {
                app.ensure_loaded(Page::Album(id));
            }
        }
        FindPage::Playlist { id, .. } => {
            if !app.playlist_pages.contains_key(&id) {
                app.ensure_loaded(Page::Playlist(id));
            }
        }
        FindPage::Artist { id, .. } => {
            let held = app
                .artist_pages
                .get(&id)
                .is_some_and(|page| page.albums.contains_key(page.filter.groups()));
            if !held {
                app.ensure_loaded(Page::Artist(id));
            }
        }
        FindPage::Radio { seed, .. } => {
            if !app.radio_pages.contains_key(&seed) {
                app.ensure_loaded(Page::Radio(seed));
            }
        }
    }
}

/// Whether the list is scrolled near enough its end to ask for more.
fn near_end(len: usize, scroll: usize, in_view: usize) -> bool {
    scroll + in_view + in_view / 2 >= len
}

/// The list's lines: the rows, numbered as the playlist's, then the error
/// a page's load ended in, in the list's "current" colour.
fn lines(app: &App, listing: &Listing) -> Vec<ListRow<'static>> {
    let playing = app.now_playing().map(|now| now.uri);
    let selected = app.winamp.find.selected;
    let mut lines: Vec<ListRow> = listing
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| ListRow {
            label: Cow::Owned(format!("{}. {}", index + 1, row.label)),
            right: row.duration_ms.map(util::format_duration_ms),
            current: playing.as_deref() == Some(row.uri.as_str()),
            selected: selected == Some(index),
        })
        .collect();
    if let Some((error, _)) = &listing.failed {
        let error = crate::ui::widgets::sanitize_error_message(error, app.locale);
        lines.push(ListRow {
            label: Cow::Owned(format!("{error} - {}", gettext(app.locale, "Retry"))),
            right: None,
            current: true,
            selected: false,
        });
    }
    lines
}

/// A click on a line: a row is picked; the error line loads the page again.
fn pick(app: &mut App, listing: &Listing, index: usize) {
    if index < listing.rows.len() {
        app.winamp.find.selected = Some(index);
    } else if let Some((_, page)) = &listing.failed {
        app.actions.push(Action::Reload(page.clone()));
    }
}

/// Keeps each half of the answer for what the box holds as it arrives, to
/// show while the next answer is awaited. The selection goes only when the
/// rows it was on are replaced.
fn hold_answer(app: &mut App) {
    let (catalogue, lists) = arrived(app);
    if !catalogue && !lists {
        return;
    }
    let Loadable::Loaded(results) = &app.search.results else {
        return;
    };
    let find = &mut app.winamp.find;
    if find
        .held
        .take(app.search.serial, results, catalogue, lists, find.mode)
    {
        find.selected = None;
    }
}

/// Double-click, Enter or PLAY on the `index`th row: a song plays, and
/// an artist, album or playlist opens inside FIND.
fn choose(app: &mut App, rows: &[FindRow], index: usize) {
    let Some(row) = rows.get(index) else {
        return;
    };
    match app.winamp.find.choose(row, rows, index) {
        Choice::Play(action) => app.actions.push(action),
        Choice::Open(page) => {
            app.winamp.find.open(page);
            app.winamp.find_scroll = 0;
        }
    }
}

/// +QUEUE on a row.
fn queue(app: &mut App, row: &FindRow) {
    if row.kind == ItemKind::Song {
        app.actions.push(Action::AddToQueue {
            uri: row.uri.clone(),
            label: row.label.clone(),
        });
    }
}

/// Whether FIND is drawn open, so it can hold the keyboard.
fn open(app: &App) -> bool {
    app.settings.section_open(WinampSection::Find)
        && !app.settings.section_shaded(WinampSection::Find)
}

/// The keyboard, before the mini player's own keys see it: while FIND has
/// focus it takes typed text, pastes, and every key without Cmd (Ctrl
/// elsewhere), so M types an "m" rather than muting. Cmd shortcuts still
/// reach the window, except Cmd+Backspace, which empties the box. Also runs
/// the search a pause in typing has made due.
pub(super) fn keys(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    if open(app) {
        if std::mem::take(&mut app.winamp.find.focus_requested) {
            app.winamp.find.focused = true;
        }
    } else {
        app.winamp.find.focused = false;
    }
    if app.winamp.find.focused {
        for event in take_keys(ctx) {
            key(app, event, now);
        }
    }
    let due = app.winamp.find.due(now);
    app.actions.extend(due);
    if let Some(wait) = app.winamp.find.wait(now) {
        ctx.request_repaint_after(wait);
    }
}

/// Takes this frame's typing out of egui's input.
fn take_keys(ctx: &egui::Context) -> Vec<Event> {
    ctx.input_mut(|input| {
        let mut taken = Vec::new();
        input.events.retain(|event| {
            let take = match event {
                Event::Text(_) | Event::Paste(_) => true,
                Event::Key { key, modifiers, .. } => !modifiers.command || *key == Key::Backspace,
                _ => false,
            };
            if take {
                taken.push(event.clone());
            }
            !take
        });
        taken
    })
}

/// One taken event.
fn key(app: &mut App, event: Event, now: Instant) {
    match event {
        Event::Text(text) | Event::Paste(text) => {
            let text: String = text
                .chars()
                .map(|c| if c.is_whitespace() { ' ' } else { c })
                .filter(|c| !c.is_control())
                .collect();
            let query = app.winamp.find.query.clone() + &text;
            retype(app, query, now);
        }
        Event::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } => match key {
            Key::Backspace if modifiers.command || modifiers.ctrl => {
                let actions = app.winamp.find.set_query(String::new());
                app.actions.extend(actions);
                app.winamp.find_scroll = 0;
            }
            // Only a fresh press: holding Backspace to empty the box stops
            // once it is empty rather than going on through the pages.
            Key::Backspace if app.winamp.find.query.is_empty() => {
                if !repeat {
                    back(app);
                }
            }
            Key::Backspace => {
                let mut query = app.winamp.find.query.clone();
                query.pop();
                retype(app, query, now);
            }
            Key::Enter => {
                let find = &mut app.winamp.find;
                if find.pending.is_some() || find.selected.is_none() {
                    let actions = find.submit();
                    app.actions.extend(actions);
                } else if let Some(index) = find.selected {
                    let rows = visible_rows(app);
                    choose(app, &rows, index);
                }
            }
            Key::Escape => app.winamp.find.focused = false,
            Key::ArrowDown => step(app, 1),
            Key::ArrowUp => step(app, -1),
            Key::Tab => {
                app.winamp.find.cycle_mode(!modifiers.shift);
                app.winamp.find_scroll = 0;
            }
            _ => {}
        },
        _ => {}
    }
}

/// BACK, or Backspace in an empty box: out of the page FIND is in, to
/// the row it was opened from.
fn back(app: &mut App) {
    if app.winamp.find.back() {
        app.winamp.find_scroll = 0;
        if let Some(index) = app.winamp.find.selected {
            reveal(app, index);
        }
    }
}

/// The box changed under the user's keys.
fn retype(app: &mut App, query: String, now: Instant) {
    if query != app.winamp.find.query {
        app.winamp.find.type_query(query, now);
        app.winamp.find_scroll = 0;
    }
}

/// The rows a list area of the section's height has in view.
fn rows_in_view(app: &App) -> usize {
    let height = app.settings.section_height(WinampSection::Find);
    (split(section::list_area(height)).2.height / ROW) as usize
}

/// Moves the selection a row, keeping it in view.
fn step(app: &mut App, delta: i64) {
    let count = visible_rows(app).len();
    if count == 0 {
        return;
    }
    let last = count as i64 - 1;
    let index = match app.winamp.find.selected {
        None if delta > 0 => 0,
        None => last,
        Some(index) => (index as i64 + delta).clamp(0, last),
    } as usize;
    app.winamp.find.selected = Some(index);
    reveal(app, index);
}

/// Scrolls the list so the `index`th row is in view.
fn reveal(app: &mut App, index: usize) {
    let in_view = rows_in_view(app).max(1);
    let scroll = &mut app.winamp.find_scroll;
    if index < *scroll {
        *scroll = index;
    } else if index >= *scroll + in_view {
        *scroll = index + 1 - in_view;
    }
}

/// The list's area cut into the box, the mode row and the list.
fn split(area: Area) -> (Area, Area, Area) {
    let bar = Area::new(area.x, area.y, area.width, ROW.min(area.height));
    let modes = Area::new(
        area.x,
        area.y + bar.height,
        area.width,
        ROW.min(area.height - bar.height),
    );
    let top = bar.height + modes.height;
    let list = Area::new(area.x, area.y + top, area.width, area.height - top);
    (bar, modes, list)
}

/// The section, under the sections above it. `focused` is the window's.
pub(super) fn show(app: &mut App, view: &mut View, focused: bool) {
    let title = app.winamp.find.title();
    let Some(chrome) = section::frame(app, view, WinampSection::Find, &title, focused) else {
        app.winamp.find.focused = false;
        return;
    };
    let height = chrome.height;
    let (bar, modes, area) = split(section::list_area(height));
    take_focus(app, view, bar);
    hold_answer(app);
    fetch(app);
    input_bar(app, view, bar, focused);
    mode_row(app, view, modes);

    let listing = listing(app, app.winamp.find.page());
    let lines = lines(app, &listing);
    let winamp = &mut app.winamp;
    let response = section::list(
        view,
        &mut winamp.playlist_text,
        &lines,
        &mut winamp.find_scroll,
        &mut winamp.find_wheel,
        area,
        WinampSection::Find,
    );
    if let Some(index) = response.clicked.or(response.secondary) {
        pick(app, &listing, index);
    }
    let rows = &listing.rows;
    if let Some(index) = response.double_clicked {
        choose(app, rows, index);
    }
    // Library lists have no cap: their next page comes as the end nears.
    if let Some(more) = listing.more.clone()
        && near_end(rows.len(), app.winamp.find_scroll, rows_in_view(app))
    {
        app.actions.push(more);
    }

    let chosen = app.winamp.find.selected.filter(|index| *index < rows.len());
    match section::bottom_buttons(view, height, &BUTTONS, "find") {
        Some(PLAY) => {
            if let Some(index) = chosen {
                choose(app, rows, index);
            }
        }
        Some(QUEUE) => {
            if let Some(row) = chosen.and_then(|index| rows.get(index)) {
                queue(app, row);
            }
        }
        Some(BACK) => back(app),
        _ => {}
    }
}

/// Only a press on the box gives it the keyboard; a press anywhere else
/// in the window, the list and the buttons included, takes it back, so
/// Space still plays and pauses once a row is picked.
fn take_focus(app: &mut App, view: &View, bar: Area) {
    let bar = view.rect(bar);
    let pressed = view.ui.ctx().input(|input| {
        input
            .pointer
            .any_pressed()
            .then(|| input.pointer.press_origin())
            .flatten()
    });
    if let Some(origin) = pressed {
        app.winamp.find.focused = bar.contains(origin);
    }
}

/// The skin's list colour as egui's.
fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A line of the playlist's font in a colour, from the left of `at` and
/// centred in its height, cut at its right. Returns how wide it drew.
fn write(view: &View, text: &mut PixelText, label: &str, at: Area, color: Color32) -> u32 {
    let ctx = view.ui.ctx().clone();
    let line = text.line(&ctx, label);
    let width = line.width.min(at.width);
    let dest = Area::new(
        at.x,
        at.y + at.height.saturating_sub(line.height) / 2,
        width,
        line.height,
    );
    let uv = egui::Rect::from_min_max(
        egui::pos2(0.0, 0.0),
        egui::pos2(width as f32 / line.width.max(1) as f32, 1.0),
    );
    let clip = view.rect(at).intersect(view.ui.clip_rect());
    view.ui
        .painter()
        .with_clip_rect(clip)
        .image(line.texture.id(), view.rect(dest), uv, color);
    width
}

/// The end of the text that fits `width`, so the caret stays in view.
fn tail(text: &mut PixelText, label: &str, width: f32) -> String {
    let mut chars = label.chars();
    while text.width(chars.as_str()) > width && chars.next().is_some() {}
    chars.as_str().to_string()
}

/// The box: its edge, the query or a hint, the caret while it has the
/// keyboard, and the X that empties it.
fn input_bar(app: &mut App, view: &mut View, bar: Area, window_focused: bool) {
    let style = view.skin.playlist.clone();
    let ink = rgb(style.normal);
    let edge = ink.gamma_multiply(0.5);
    view.fill(bar.x, bar.y, bar.width, 1, edge);
    view.fill(bar.x, bar.y + bar.height - 1, bar.width, 1, edge);
    view.fill(bar.x, bar.y, 1, bar.height, edge);
    view.fill(bar.x + bar.width - 1, bar.y, 1, bar.height, edge);

    let cross = Area::new(bar.x + bar.width - ROW, bar.y, ROW, bar.height);
    let field = Area::new(bar.x + 3, bar.y, cross.x - bar.x - 3, bar.height);
    let find = &app.winamp.find;
    let typing = find.focused;
    let empty = find.query.is_empty();
    let query = find.query.clone();
    let text = &mut app.winamp.playlist_text;
    if empty && !typing {
        let hint = gettext(app.locale, "Search Spotify");
        write(view, text, &hint, field, ink.gamma_multiply(0.5));
    } else {
        // Room for the caret after the text.
        let shown = tail(text, &query, field.width.saturating_sub(2) as f32);
        let width = if shown.is_empty() {
            0
        } else {
            write(view, text, &shown, field, ink)
        };
        if typing && window_focused {
            let time = view.ui.ctx().input(|input| input.time);
            let half = BLINK.as_secs_f64();
            if ((time / half) as u64).is_multiple_of(2) {
                view.fill(field.x + width + 1, bar.y + 2, 1, bar.height - 4, ink);
            }
            let next = half - time % half;
            view.ui
                .ctx()
                .request_repaint_after(Duration::from_secs_f64(next));
        }
    }

    if !empty {
        // A five-pixel X, a pixel at a time so it stays sharp at any scale.
        let (x, y) = (cross.x + (ROW - 5) / 2, cross.y + (cross.height - 5) / 2);
        for step in 0..5 {
            view.fill(x + step, y + step, 1, 1, ink);
            view.fill(x + 4 - step, y + step, 1, 1, ink);
        }
        if view.interact(cross, "find-clear", Sense::click()).clicked() {
            let actions = app.winamp.find.set_query(String::new());
            app.actions.extend(actions);
            app.winamp.find_scroll = 0;
            app.winamp.find.focused = true;
        }
    }
}

/// The modes for what the box holds, or an artist's SONGS and ALBUMS on
/// their page, the one shown picked out in the playlist's selection
/// colours.
fn mode_row(app: &mut App, view: &mut View, row: Area) {
    let style = view.skin.playlist.clone();
    let gap = 8;
    let mut x = row.x + 3;
    let current = app.winamp.find.shown_mode();
    for &mode in app.winamp.find.modes() {
        let label = mode.label();
        let width = app.winamp.playlist_text.width(label).ceil() as u32;
        let cell = Area::new(x - 2, row.y, width + 4, row.height);
        if cell.x + cell.width > row.x + row.width {
            break;
        }
        let picked = mode == current;
        if picked {
            view.fill(
                cell.x,
                cell.y,
                cell.width,
                cell.height,
                rgb(style.selected_background),
            );
        }
        let color = rgb(if picked { style.current } else { style.normal });
        write(
            view,
            &mut app.winamp.playlist_text,
            label,
            Area::new(x, row.y, width, row.height),
            color,
        );
        if view
            .interact(cell, &format!("find-mode-{label}"), Sense::click())
            .clicked()
        {
            // Inside an album, playlist or radio, every mode leads out.
            let leaves = !matches!(
                app.winamp.find.page(),
                FindPage::Top | FindPage::Artist { .. }
            );
            if mode != current || leaves {
                app.winamp.find.pick_mode(mode);
                app.winamp.find_scroll = 0;
            }
        }
        x += width + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::Loadable;

    fn test_app() -> App {
        let root = std::env::temp_dir().join(format!(
            "spotifast-find-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        App::new(
            &crate::backend::Waker::default(),
            crate::paths::AppDirs {
                config: root.join("config"),
                state: root.join("state"),
                cache: root.join("cache"),
            },
            crate::settings::Settings::default(),
            crate::app::AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
                autostart: false,
            },
        )
    }

    /// The demo's search for "Bonobo", answered.
    fn searched_app() -> App {
        let mut app = test_app();
        crate::demo::populate(&mut app);
        assert_eq!(app.search.committed, "Bonobo");
        assert!(matches!(app.search.results, Loadable::Loaded(_)));
        app
    }

    #[test]
    fn an_answer_for_an_older_query_is_not_shown() {
        let mut app = searched_app();
        // Typed on past the query the answer is for: nothing was shown
        // before, so nothing is shown now.
        app.winamp.find.set_query("Bonobo live".into());
        assert!(visible_rows(&app).is_empty());

        // The answer for what the box holds is shown.
        app.winamp.find.set_query("Bonobo".into());
        let shown = visible_rows(&app);
        assert!(!shown.is_empty());
        assert!(shown.iter().all(|row| row.kind == state::ItemKind::Song));
        hold_answer(&mut app);

        // Typing on keeps those rows rather than emptying the list, and
        // they never become another query's rows: the main window's search
        // for something else changes nothing here.
        app.winamp.find.set_query("Bonobo live".into());
        assert_eq!(visible_rows(&app), shown);
        app.search.committed = "Khruangbin".into();
        assert_eq!(visible_rows(&app), shown);

        // Emptying the box lets go of them.
        app.winamp.find.set_query(String::new());
        hold_answer(&mut app);
        app.winamp.find.set_query("Bonobo live".into());
        assert!(visible_rows(&app).is_empty());
        app.backend.shutdown();
    }

    #[test]
    fn choosing_a_song_plays_it_alone_and_queue_adds_it() {
        let mut app = searched_app();
        app.winamp.find.set_query("Bonobo".into());
        let rows = visible_rows(&app);
        app.actions.clear();
        choose(&mut app, &rows, 1);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::PlayUris { uris, index: 0 }] if uris == &[rows[1].uri.clone()]
        ));
        app.actions.clear();
        queue(&mut app, &rows[1]);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::AddToQueue { uri, .. }] if uri == &rows[1].uri
        ));
        app.backend.shutdown();
    }
    fn press(key: Key, modifiers: egui::Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// A frame of input through FIND, then the window's own keys.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                keys(app, ui.ctx());
                crate::ui::keys::handle(app, ui.ctx());
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn the_box_takes_typing_from_the_window_shortcuts_while_focused() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.settings.find_open = true;
        let none = egui::Modifiers::NONE;
        // Unfocused, M is the window's: mute.
        frame(
            &mut app,
            &ctx,
            vec![press(Key::M, none), Event::Text("m".into())],
        );
        assert!(matches!(app.actions.as_slice(), [Action::ToggleMute]));
        assert!(app.winamp.find.query.is_empty());
        app.actions.clear();

        app.winamp.find.focus_requested = true;
        frame(
            &mut app,
            &ctx,
            vec![
                press(Key::M, none),
                Event::Text("m".into()),
                press(Key::Space, none),
                Event::Text(" ".into()),
                Event::Paste("x\ny".into()),
            ],
        );
        assert!(app.winamp.find.focused);
        assert_eq!(app.winamp.find.query, "m x y");
        assert_eq!(app.winamp.find.mode, state::FindMode::Songs);
        assert!(app.actions.is_empty(), "{:?}", app.actions);

        frame(&mut app, &ctx, vec![press(Key::Backspace, none)]);
        assert_eq!(app.winamp.find.query, "m x ");
        frame(&mut app, &ctx, vec![press(Key::Enter, none)]);
        assert!(matches!(app.actions.as_slice(), [Action::FindSearch(q)] if q == "m x"));
        app.actions.clear();

        frame(
            &mut app,
            &ctx,
            vec![press(Key::Backspace, egui::Modifiers::COMMAND)],
        );
        assert!(app.winamp.find.query.is_empty());
        assert_eq!(app.winamp.find.mode, state::FindMode::Library);

        // Escape gives the keys back to the window.
        frame(&mut app, &ctx, vec![press(Key::Escape, none)]);
        assert!(!app.winamp.find.focused);
        frame(&mut app, &ctx, vec![press(Key::M, none)]);
        assert!(matches!(app.actions.as_slice(), [Action::ToggleMute]));

        // A closed FIND never holds the keyboard.
        app.winamp.find.focused = true;
        app.settings.find_open = false;
        frame(&mut app, &ctx, vec![]);
        assert!(!app.winamp.find.focused);
        app.backend.shutdown();
    }

    #[test]
    fn arrows_walk_the_rows_and_enter_plays_the_selected_song() {
        let ctx = egui::Context::default();
        let mut app = searched_app();
        app.settings.find_open = true;
        let actions = app.winamp.find.set_query("Bonobo".into());
        assert!(matches!(actions.as_slice(), [Action::FindSearch(_)]));
        app.winamp.find.focused = true;
        app.actions.clear();
        let none = egui::Modifiers::NONE;
        frame(
            &mut app,
            &ctx,
            vec![press(Key::ArrowDown, none), press(Key::ArrowDown, none)],
        );
        assert_eq!(app.winamp.find.selected, Some(1));
        frame(&mut app, &ctx, vec![press(Key::Enter, none)]);
        let rows = visible_rows(&app);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::PlayUris { uris, index: 0 }] if uris == &[rows[1].uri.clone()]
        ));
        app.actions.clear();
        frame(&mut app, &ctx, vec![press(Key::Tab, none)]);
        assert_eq!(app.winamp.find.mode, state::FindMode::Artists);
        assert_eq!(app.winamp.find.selected, None);
        app.backend.shutdown();
    }
    /// Half an answer is not the answer: the rows on screen stay until the
    /// mode's own kind arrives, and what is kept has every kind.
    #[test]
    fn half_an_answer_neither_blanks_the_list_nor_is_kept_as_the_whole() {
        for playlists_first in [true, false] {
            let mut app = searched_app();
            let demo = match &app.search.results {
                Loadable::Loaded(results) => results.clone(),
                _ => unreachable!(),
            };
            // Bonobo's songs on screen.
            app.winamp.find.set_query("Bonobo".into());
            hold_answer(&mut app);
            let songs = visible_rows(&app);
            assert!(!songs.is_empty());

            // A new search whose halves land one at a time.
            app.search.results = Loadable::Loaded(demo.clone());
            let actions = app.winamp.find.set_query("Bonobo live".into());
            let ctx = egui::Context::default();
            for action in actions {
                app.apply(action, &ctx);
            }
            // A held row picked while the new answer is on its way.
            app.winamp.find.selected = Some(2);
            let serial = app.search.serial;
            app.handle_api(crate::backend::ApiResponse::SearchStarted {
                query: "Bonobo live".into(),
                serial,
                split: true,
            });
            let mut catalogue = demo.clone();
            let lists = catalogue.playlists.take().unwrap();
            let catalogue = crate::backend::ApiResponse::Search {
                query: "Bonobo live".into(),
                serial,
                result: Ok(catalogue),
            };
            let playlists = crate::backend::ApiResponse::SearchPlaylists {
                query: "Bonobo live".into(),
                serial,
                result: Ok(lists),
            };
            let (first, second) = if playlists_first {
                (playlists, catalogue)
            } else {
                (catalogue, playlists)
            };
            app.handle_api(first);
            hold_answer(&mut app);
            if playlists_first {
                assert_eq!(visible_rows(&app), songs, "playlists alone keep the songs");
                assert_eq!(app.winamp.find.selected, Some(2));
            }
            app.handle_api(second);
            hold_answer(&mut app);
            assert_eq!(visible_rows(&app).len(), songs.len());

            // Typing past it keeps every kind of the whole answer.
            app.winamp.find.set_query("Bonobo live at".into());
            assert_eq!(visible_rows(&app).len(), songs.len());
            app.winamp.find.set_mode(state::FindMode::Lists);
            assert!(!visible_rows(&app).is_empty(), "LISTS kept the playlists");
            app.backend.shutdown();
        }
    }

    /// Draws FIND once into a context, for pointer tests.
    fn draw(app: &mut App, ctx: &egui::Context, events: Vec<Event>) {
        let skin = app.winamp.skin.clone();
        let textures = std::collections::HashMap::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                let mut view = View {
                    ui,
                    origin: egui::pos2(0.0, 0.0),
                    unit: 1.0,
                    skin: &skin,
                    textures: &textures,
                    mask: None,
                };
                show(app, &mut view, true);
            },
        );
        output.textures_delta.clear();
    }

    /// A click at a point in skin pixels, over two frames.
    fn click(app: &mut App, ctx: &egui::Context, x: f32, y: f32) {
        let pos = egui::pos2(x, y);
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw(app, ctx, vec![Event::PointerMoved(pos)]);
        draw(app, ctx, vec![button(true)]);
        draw(app, ctx, vec![button(false)]);
    }

    /// Picking a row leaves the keys with the window, so Space still plays
    /// and pauses; only the box itself takes them.
    #[test]
    fn only_the_box_takes_the_keyboard() {
        let ctx = egui::Context::default();
        let mut app = searched_app();
        app.settings.find_open = true;
        app.winamp.find.set_query("Bonobo".into());
        app.winamp.find.focused = true;
        let area = section::list_area(app.settings.section_height(WinampSection::Find));
        // The second row of the list, under the box and the mode row.
        let row = (area.y + 2 * ROW + ROW + ROW / 2) as f32;
        click(&mut app, &ctx, 60.0, row);
        assert_eq!(app.winamp.find.selected, Some(1));
        assert!(!app.winamp.find.focused, "a row is not the box");
        assert_eq!(app.winamp.find.query, "Bonobo");

        click(&mut app, &ctx, 60.0, (area.y + ROW / 2) as f32);
        assert!(app.winamp.find.focused, "the box takes the keys");
        assert_eq!(app.winamp.find.selected, Some(1));

        // The bottom bar is not the box either.
        let height = app.settings.section_height(WinampSection::Find);
        click(&mut app, &ctx, 30.0, (height - 19) as f32);
        assert!(!app.winamp.find.focused);
        app.backend.shutdown();
    }
    fn kinds(rows: &[FindRow]) -> Vec<ItemKind> {
        let mut kinds: Vec<ItemKind> = rows.iter().map(|row| row.kind).collect();
        kinds.dedup();
        kinds
    }

    #[test]
    fn library_lists_playlists_albums_artists_then_liked_songs() {
        let mut app = searched_app();
        let rows = visible_rows(&app);
        assert_eq!(
            kinds(&rows),
            [
                ItemKind::Playlist,
                ItemKind::Album,
                ItemKind::Artist,
                ItemKind::Song
            ]
        );
        let liked = app.library.liked.items.len();
        assert_eq!(
            rows.iter().filter(|r| r.kind == ItemKind::Song).count(),
            liked
        );
        // Whole, the songs play in the collection, as the big window does.
        let song = rows.iter().find(|r| r.kind == ItemKind::Song).unwrap();
        assert_eq!(
            song.context.as_deref(),
            Some("spotify:user:demo:collection")
        );
        // No cap: a long library lists every row.
        assert!(rows.len() > state::RESULT_LIMIT);

        // A list still loading ends the rows there, and its next page is
        // what scrolling near the end asks for.
        app.library.albums.next_offset = Some(8);
        let listing = listing(&app, &FindPage::Top);
        assert_eq!(kinds(&listing.rows), [ItemKind::Playlist, ItemKind::Album]);
        assert!(matches!(listing.more, Some(Action::LoadMore(Page::Albums))));
        let len = listing.rows.len();
        assert!(!near_end(len, 0, 5));
        assert!(near_end(len, len - 5, 5));
        // Already on its way: not asked for twice.
        app.library.albums.loading = true;
        assert!(listing_more(&app).is_none());
        app.backend.shutdown();
    }

    fn listing_more(app: &App) -> Option<Action> {
        listing(app, &FindPage::Top).more
    }

    #[test]
    fn recent_lists_the_songs_played_lately() {
        let mut app = searched_app();
        app.winamp.find.set_mode(state::FindMode::Recent);
        let rows = visible_rows(&app);
        let Loadable::Loaded(history) = &app.home.recently_played else {
            unreachable!()
        };
        assert_eq!(rows.len(), history.len());
        assert_eq!(rows[0].uri, history[0].track.uri);
        assert!(rows.iter().all(|r| r.kind == ItemKind::Song));
        app.home.recently_played = Loadable::Loading;
        assert!(matches!(page_rows(&app, &FindPage::Top), Loadable::Loading));
        app.backend.shutdown();
    }

    #[test]
    fn a_page_that_failed_to_load_shows_one_line_and_keeps_back() {
        let mut app = searched_app();
        app.album_pages.get_mut("alb0").unwrap().album = Loadable::Failed("Gone".into());
        app.winamp.find.open(FindPage::Album {
            id: "alb0".into(),
            name: "Album".into(),
        });
        let page = app.winamp.find.page().clone();
        assert!(matches!(page_rows(&app, &page), Loadable::Failed(_)));
        let listing = listing(&app, &page);
        let lines = lines(&app, &listing);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].current, "in the list's current colour");
        app.actions.clear();
        pick(&mut app, &listing, 0);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::Reload(Page::Album(id))] if id == "alb0"
        ));
        assert_eq!(app.winamp.find.selected, None, "the line is not a row");

        back(&mut app);
        assert_eq!(app.winamp.find.page(), &FindPage::Top);
        assert!(matches!(
            page_rows(&app, &FindPage::Top),
            Loadable::Loaded(rows) if kinds(&rows).len() == 4
        ));
        app.backend.shutdown();
    }

    #[test]
    fn open_an_artist_then_an_album_and_back_twice() {
        let mut app = searched_app();
        let rows = visible_rows(&app);
        let artist = rows
            .iter()
            .position(|r| r.uri == "spotify:artist:art0")
            .unwrap();
        app.winamp.find.selected = Some(artist);
        app.winamp.find_scroll = 3;
        choose(&mut app, &rows, artist);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Artist { id, albums: false, .. } if id == "art0"
        ));
        assert_eq!(app.winamp.find_scroll, 0);
        let songs = visible_rows(&app);
        assert_eq!(songs.len(), 10);
        app.actions.clear();
        choose(&mut app, &songs, 3);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::PlayUris { uris, index: 3 }] if uris.len() == 10
        ));

        app.winamp.find.pick_mode(state::FindMode::Albums);
        let albums = visible_rows(&app);
        assert!(!albums.is_empty());
        assert!(albums.iter().all(|r| r.kind == ItemKind::Album));
        let first = albums
            .iter()
            .position(|r| r.uri == "spotify:album:alb0")
            .unwrap();
        choose(&mut app, &albums, first);
        assert!(app.winamp.find.title().starts_with("FIND - "));
        let tracks = visible_rows(&app);
        assert_eq!(tracks.len(), 12);
        app.actions.clear();
        choose(&mut app, &tracks, 2);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::PlayContext { uri, offset_uri: Some(o), .. }]
                if uri == "spotify:album:alb0" && *o == tracks[2].uri
        ));

        back(&mut app);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Artist { albums: true, .. }
        ));
        assert_eq!(app.winamp.find.selected, None);
        back(&mut app);
        assert_eq!(app.winamp.find.page(), &FindPage::Top);
        assert_eq!(app.winamp.find.selected, Some(artist), "back to the row");
        app.backend.shutdown();
    }

    fn release(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Holding Backspace empties the box and stops there: only a fresh
    /// press goes back a page.
    #[test]
    fn a_held_backspace_never_goes_back() {
        let ctx = egui::Context::default();
        let mut app = searched_app();
        app.settings.find_open = true;
        app.winamp.find.set_query(String::new());
        app.winamp.find.open(FindPage::Artist {
            id: "art0".into(),
            name: "R".into(),
            albums: true,
        });
        app.winamp.find.open(FindPage::Album {
            id: "alb0".into(),
            name: "A".into(),
        });
        app.winamp.find.focused = true;
        let backspace = || press(Key::Backspace, egui::Modifiers::NONE);
        // Held down: egui marks every press after the first as a repeat.
        frame(&mut app, &ctx, vec![backspace()]);
        assert!(matches!(app.winamp.find.page(), FindPage::Artist { .. }));
        frame(&mut app, &ctx, vec![backspace()]);
        frame(&mut app, &ctx, vec![backspace()]);
        assert!(
            matches!(app.winamp.find.page(), FindPage::Artist { .. }),
            "the repeats stay"
        );
        // Let go, then a fresh press.
        frame(&mut app, &ctx, vec![release(Key::Backspace)]);
        frame(&mut app, &ctx, vec![backspace()]);
        assert_eq!(app.winamp.find.page(), &FindPage::Top);
        app.backend.shutdown();
    }
}
