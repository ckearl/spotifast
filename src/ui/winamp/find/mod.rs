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

use crate::app::App;
use crate::i18n::gettext;
use crate::model::{Action, Loadable, WinampSection};
use crate::skin::layout::{self, Area};
use crate::util;

use super::section::{self, ListRow};
use super::{PixelText, View};

pub mod state;

pub use state::FindState;
use state::{FindRow, ItemKind};

/// One row of text, as the playlist's.
const ROW: u32 = layout::PLAYLIST_TRACK_HEIGHT;
/// The bottom bar. +LIST and RADIO arrive with the actions and pages.
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

/// The rows the list shows. The answer is shown only for what the box
/// holds, and only once the mode's own half of it has come; until then the
/// last answer shown stays, so the rows never belong to a query the user
/// has typed past, nor flicker away.
pub(super) fn visible_rows(app: &App) -> Vec<FindRow> {
    let find = &app.winamp.find;
    // The library, recent songs and the pages come with their own lists.
    if find.stack.len() > 1 || !find.mode.is_search() {
        return Vec::new();
    }
    let (catalogue, lists) = arrived(app);
    let live = if find.mode == state::FindMode::Lists {
        lists
    } else {
        catalogue
    };
    match &app.search.results {
        Loadable::Loaded(results) if live => find.top_search_rows(results),
        _ => find.top_search_rows(&find.held.results),
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

/// Double-click, Enter or PLAY on a row. A song plays on its own and
/// Spotify's autoplay carries on after it; the other kinds open inside
/// FIND once it has pages.
fn choose(app: &mut App, row: &FindRow) {
    if row.kind == ItemKind::Song {
        app.actions.push(Action::PlayUris {
            uris: vec![row.uri.clone()],
            index: 0,
        });
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

/// The selected row, if it is still on the list.
fn selected_row(app: &App) -> Option<FindRow> {
    let index = app.winamp.find.selected?;
    visible_rows(app).into_iter().nth(index)
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
            modifiers,
            ..
        } => match key {
            Key::Backspace if modifiers.command || modifiers.ctrl => {
                let actions = app.winamp.find.set_query(String::new());
                app.actions.extend(actions);
                app.winamp.find_scroll = 0;
            }
            Key::Backspace if app.winamp.find.query.is_empty() => back(app),
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
                } else if let Some(row) = selected_row(app) {
                    choose(app, &row);
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

/// BACK, or Backspace in an empty box: out of the page FIND is in.
fn back(app: &mut App) {
    if app.winamp.find.back() {
        app.winamp.find_scroll = 0;
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
    input_bar(app, view, bar, focused);
    mode_row(app, view, modes);

    let rows = visible_rows(app);
    let playing = app.now_playing().map(|now| now.uri);
    let selected = app.winamp.find.selected;
    let lines: Vec<ListRow> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| ListRow {
            label: Cow::Owned(format!("{}. {}", index + 1, row.label)),
            right: row.duration_ms.map(util::format_duration_ms),
            current: playing.as_deref() == Some(row.uri.as_str()),
            selected: selected == Some(index),
        })
        .collect();
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
        winamp.find.selected = Some(index);
    }
    if let Some(row) = response.double_clicked.and_then(|index| rows.get(index)) {
        choose(app, row);
    }

    let chosen = app.winamp.find.selected.and_then(|index| rows.get(index));
    match section::bottom_buttons(view, height, &BUTTONS, "find") {
        Some(PLAY) => {
            if let Some(row) = chosen {
                choose(app, row);
            }
        }
        Some(QUEUE) => {
            if let Some(row) = chosen {
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

/// The modes for what the box holds, the one shown picked out in the
/// playlist's selection colours.
fn mode_row(app: &mut App, view: &mut View, row: Area) {
    let style = view.skin.playlist.clone();
    let gap = 8;
    let mut x = row.x + 3;
    let current = app.winamp.find.mode;
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
            app.winamp.find.set_mode(mode);
            app.winamp.find_scroll = 0;
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
        choose(&mut app, &rows[1]);
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
}
