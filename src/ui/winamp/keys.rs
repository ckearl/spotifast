//! The mini player's keys: one key for each section, and two-key
//! sequences in the style of `spotify_player` for the item they act on.
//!
//! They come between FIND's box, which keeps every key typed into it, and
//! the window's own keys (`crate::ui::keys`), which get what is left:
//! Space, M, S, R, B and ? among them.

use std::time::{Duration, Instant};

use egui::{Event, Key, Modifiers};

use crate::app::{App, NowPlaying};
use crate::i18n::gettext;
use crate::model::{Action, RowContext, WinampSection};
use crate::util;

use super::find::state::{FindPage, FindRow};
use super::item_menu::{self, ItemAction};
use super::{find, playlist};

/// How long the first key of a sequence waits for the second.
pub(super) const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(1);

/// The first key of a sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verb {
    Go,
    Add,
    Remove,
}

/// What a key or a sequence does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Section(WinampSection),
    Eq,
    Playlist,
    FocusFind,
    Item(ItemAction),
    /// Like, save or follow (`true`), or undo it, whichever the item takes.
    SetSaved(bool),
    Recent,
    Library,
    Devices,
    ShowPlaying,
    RemoveFromQueue,
}

/// The keys pressed so far: a verb waiting for its second key.
#[derive(Debug, Default)]
pub struct Sequencer {
    pending: Option<(Verb, Instant)>,
}

impl Sequencer {
    /// A key pressed at `now`: the command it ends, if any. A verb waits
    /// for its second key; a second key the verb has no use for ends the
    /// sequence and does nothing else.
    pub(super) fn press(&mut self, key: char, now: Instant) -> Option<Command> {
        if let Some((verb, at)) = self.pending.take()
            && now.saturating_duration_since(at) <= SEQUENCE_TIMEOUT
        {
            return second(verb, key);
        }
        if let Some(verb) = verb(key) {
            self.pending = Some((verb, now));
            return None;
        }
        single(key)
    }

    /// The verb waiting, as the marquee shows it.
    pub(super) fn pending_label(&self) -> Option<&'static str> {
        self.pending.map(|(verb, _)| match verb {
            Verb::Go => "g…",
            Verb::Add => "a…",
            Verb::Remove => "d…",
        })
    }

    /// Forgets a verb that has waited too long.
    fn expire(&mut self, now: Instant) {
        if self
            .pending
            .is_some_and(|(_, at)| now.saturating_duration_since(at) > SEQUENCE_TIMEOUT)
        {
            self.pending = None;
        }
    }

    /// How long the verb waiting has left, to wake and clear the marquee.
    fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending.map(|(_, at)| {
            (SEQUENCE_TIMEOUT + Duration::from_millis(1))
                .saturating_sub(now.saturating_duration_since(at))
        })
    }

    /// Whether `key` begins something here.
    fn starts(key: char) -> bool {
        verb(key).is_some() || single(key).is_some()
    }
}

fn verb(key: char) -> Option<Verb> {
    match key {
        'g' => Some(Verb::Go),
        'a' => Some(Verb::Add),
        'd' => Some(Verb::Remove),
        _ => None,
    }
}

fn single(key: char) -> Option<Command> {
    Some(match key {
        '/' => Command::FocusFind,
        'e' => Command::Eq,
        'q' => Command::Playlist,
        'l' => Command::Section(WinampSection::Lyrics),
        'v' => Command::Section(WinampSection::Vis),
        'y' => Command::Item(ItemAction::CopyLink),
        _ => return None,
    })
}

fn second(verb: Verb, key: char) -> Option<Command> {
    use Command::*;
    use ItemAction::*;
    Some(match (verb, key) {
        (Verb::Go, 'a') => Item(GoToArtist),
        (Verb::Go, 'b') => Item(GoToAlbum),
        (Verb::Go, 'r') => Item(Radio),
        (Verb::Go, 'h') => Recent,
        (Verb::Go, 'y') => Library,
        (Verb::Go, 'd') => Devices,
        (Verb::Go, 'p') => ShowPlaying,
        (Verb::Add, 'q') => Item(AddToQueue),
        (Verb::Add, 'n') => Item(PlayNext),
        (Verb::Add, 'p') => Item(AddToPlaylist),
        (Verb::Add, 'l') => SetSaved(true),
        (Verb::Remove, 'l') => SetSaved(false),
        (Verb::Remove, 'p') => Item(RemoveFromPlaylist),
        (Verb::Remove, 'q') => RemoveFromQueue,
        _ => return None,
    })
}

/// The character a plain key stands for here: a letter without Shift, or
/// the slash, which some layouts type with Shift.
fn character(key: Key, shift: bool) -> Option<char> {
    if key == Key::Slash {
        return Some('/');
    }
    let name = key.name();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) if letter.is_ascii_alphabetic() && !shift => {
            Some(letter.to_ascii_lowercase())
        }
        _ => None,
    }
}

/// Takes this frame's mini-player keys, after FIND's box has taken what
/// is typed into it and before the window's keys see the rest.
pub(super) fn handle(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    let mut commands = Vec::new();
    // FIND's box leaves Cmd keys to the window; this one is FIND's anyway.
    if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F)) {
        commands.push(Command::FocusFind);
    }
    // While FIND's box or another text field has the keyboard, the keys
    // are its: FIND's took them already, and an egui field's are its own.
    let keys = &mut app.winamp.keys;
    if app.winamp.find.focused || ctx.text_edit_focused() {
        keys.pending = None;
    } else {
        keys.expire(now);
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                let Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                if modifiers.command || modifiers.ctrl || modifiers.alt || modifiers.mac_cmd {
                    return true;
                }
                let typed = character(*key, modifiers.shift);
                // Any key but a letter ends a waiting verb and then does
                // its own job: Space plays, / focuses FIND (R16). A letter
                // the verb has no use for is swallowed.
                if !typed.is_some_and(|key| key.is_ascii_alphabetic()) {
                    keys.pending = None;
                }
                if keys.pending.is_none() && !typed.is_some_and(Sequencer::starts) {
                    return true;
                }
                // A held key repeats; only its first press counts.
                if !repeat && let Some(command) = keys.press(typed.unwrap_or('\0'), now) {
                    commands.push(command);
                }
                false
            });
        });
        if let Some(wait) = keys.wait(now) {
            ctx.request_repaint_after(wait);
        }
        // Esc steps out of FIND before the window's Esc sees it, unless a
        // menu is open for it to close.
        if !egui::Popup::is_any_open(ctx)
            && ctx.input(|input| input.key_pressed(Key::Escape))
            && find::escape(app)
        {
            ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        }
    }
    for command in commands {
        run(app, command);
    }
}

/// The row a key acts on, and the list it sits in.
struct Target {
    row: FindRow,
    context: Option<RowContext>,
    /// The playing song, which no list had picked.
    playing: bool,
}

/// The row picked in whichever of FIND and PL was picked from last, or
/// else the playing song.
fn target(app: &App) -> Option<Target> {
    let picked = if app.winamp.find_picked_last {
        find::selected_row(app)
    } else {
        playlist::selected_row(app).map(|row| (row, Some(RowContext::Queue)))
    };
    match picked {
        Some((row, context)) => Some(Target {
            row,
            context,
            playing: false,
        }),
        None => playlist::playing_row(app).map(|row| Target {
            row,
            context: None,
            playing: true,
        }),
    }
}

fn run(app: &mut App, command: Command) {
    match command {
        Command::Section(section) => app.actions.push(Action::ToggleWinampSection(section)),
        Command::Eq => app.actions.push(Action::ToggleWinampEq),
        Command::Playlist => app.actions.push(Action::ToggleWinampPlaylist),
        Command::FocusFind => find::focus_box(app),
        // A menu of its own, which the window draws on the next frame.
        Command::Item(ItemAction::AddToPlaylist) => {
            app.winamp.playlist_picker_for = target(app)
                .map(|target| target.row)
                .filter(|row| item_menu::playable(app, row).is_some());
        }
        Command::Item(action) => {
            if let Some(target) = target(app) {
                item_menu::perform(app, &target.row, action, target.context.as_ref());
            }
        }
        // Only ever towards the state asked for: `a l` on a liked song
        // leaves it liked. An unknown state counts as not saved.
        Command::SetSaved(saved) => {
            if let Some(target) = target(app)
                && app.is_saved(&target.row.uri).unwrap_or(false) != saved
            {
                item_menu::perform(
                    app,
                    &target.row,
                    ItemAction::ToggleSaved,
                    target.context.as_ref(),
                );
            }
        }
        Command::Recent => {
            find::unfold(app);
            app.winamp.find.show_recent();
            app.winamp.find_scroll = 0;
        }
        Command::Library => {
            find::unfold(app);
            app.winamp.find.show_library();
            app.winamp.find_scroll = 0;
        }
        // Task 8's devices menu opens on the next frame.
        Command::Devices => app.winamp.open_devices = true,
        Command::ShowPlaying => show_playing(app),
        Command::RemoveFromQueue => remove_from_queue(app),
    }
}

/// `d q`: PL's picked row comes out of the queue; a song picked in FIND
/// loses its first queued copy. The playing song is not in the queue.
fn remove_from_queue(app: &mut App) {
    let Some(target) = target(app).filter(|target| !target.playing) else {
        return;
    };
    let uri = target.row.uri;
    let index = match target.context {
        Some(RowContext::Queue) => target.row.position.map(|index| index as usize),
        _ => app
            .queue
            .get()
            .and_then(|queue| queue.queue.iter().position(|item| item.uri() == uri)),
    };
    if let Some(index) = index.filter(|index| app.can_remove_from_queue(*index)) {
        // The rows below move up, so the selection would name others.
        app.winamp.playlist_selection.clear();
        app.actions.push(Action::RemoveFromQueue {
            rows: vec![(index, uri)],
        });
    }
}

/// `g p`: the list the playing song plays from, opened in FIND with the
/// song picked once its rows are in; its album when the list is not one
/// FIND opens (Liked Songs, a radio, nothing).
fn show_playing(app: &mut App) {
    let Some(now) = app.now_playing() else {
        return;
    };
    let page = app
        .playing_context_uri()
        .and_then(|uri| context_page(app, &uri, &now))
        .or_else(|| {
            let id = now.album_id.clone()?;
            Some(FindPage::Album {
                id,
                name: album_name(app, now.album_id.as_deref()?, &now),
            })
        });
    let Some(page) = page else {
        return;
    };
    item_menu::open_in_find(app, page, None);
    app.winamp.find.reselect = Some(now.uri);
    app.winamp.find_picked_last = true;
}

fn album_name(app: &App, id: &str, now: &NowPlaying) -> String {
    app.album_pages
        .get(id)
        .and_then(|page| page.album.get())
        .map(|album| album.name.clone())
        .or_else(|| (now.album_id.as_deref() == Some(id)).then(|| now.album_name.clone()))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| gettext(app.locale, "Album").into_owned())
}

/// The page of FIND a context URI is: an album, a playlist or an artist.
fn context_page(app: &App, uri: &str, now: &NowPlaying) -> Option<FindPage> {
    let id = util::uri_id(uri)?.to_string();
    Some(match util::uri_kind(uri)? {
        "album" => FindPage::Album {
            name: album_name(app, &id, now),
            id,
        },
        "playlist" => FindPage::Playlist {
            name: app
                .library
                .playlists
                .get()
                .and_then(|lists| lists.iter().find(|list| list.id == id))
                .or_else(|| app.playlist_pages.get(&id)?.playlist.get())
                .map(|list| list.name.clone())
                .unwrap_or_else(|| gettext(app.locale, "Playlist").into_owned()),
            id,
        },
        "artist" => FindPage::Artist {
            name: app
                .artist_pages
                .get(&id)
                .and_then(|page| page.artist.get())
                .map(|artist| artist.name.clone())
                .or_else(|| {
                    now.artists
                        .iter()
                        .find(|artist| artist.id.as_deref() == Some(id.as_str()))
                        .map(|artist| artist.name.clone())
                })
                .unwrap_or_else(|| gettext(app.locale, "Artist").into_owned()),
            id,
            albums: false,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbs_wait_for_a_second_key() {
        let t = Instant::now();
        let mut s = Sequencer::default();
        assert_eq!(s.press('g', t), None);
        assert_eq!(s.pending_label(), Some("g…"));
        assert_eq!(s.press('a', t), Some(Command::Item(ItemAction::GoToArtist)));
        assert_eq!(s.press('a', t), None);
        assert_eq!(s.press('q', t), Some(Command::Item(ItemAction::AddToQueue)));
    }

    #[test]
    fn a_sequence_expires_after_a_second_and_unknown_keys_cancel() {
        let t = Instant::now();
        let mut s = Sequencer::default();
        s.press('g', t);
        // 'a' now starts Add.
        assert_eq!(
            s.press('a', t + SEQUENCE_TIMEOUT + Duration::from_millis(1)),
            None
        );
        assert_eq!(s.pending_label(), Some("a…"));
        assert_eq!(s.press('z', t), None);
        assert_eq!(s.pending_label(), None);
    }

    #[test]
    fn single_keys_map_to_sections() {
        let t = Instant::now();
        let mut s = Sequencer::default();
        assert_eq!(
            s.press('l', t),
            Some(Command::Section(WinampSection::Lyrics))
        );
        assert_eq!(s.press('/', t), Some(Command::FocusFind));
        assert_eq!(s.press('e', t), Some(Command::Eq));
        assert_eq!(s.press('q', t), Some(Command::Playlist));
        assert_eq!(s.press('v', t), Some(Command::Section(WinampSection::Vis)));
        assert_eq!(s.press('y', t), Some(Command::Item(ItemAction::CopyLink)));
        assert_eq!(s.press('m', t), None, "M stays the window's mute");
    }

    #[test]
    fn every_sequence_in_the_table() {
        use Command::*;
        use ItemAction::*;
        let table = [
            ("ga", Item(GoToArtist)),
            ("gb", Item(GoToAlbum)),
            ("gr", Item(Radio)),
            ("gh", Recent),
            ("gy", Library),
            ("gd", Devices),
            ("gp", ShowPlaying),
            ("aq", Item(AddToQueue)),
            ("an", Item(PlayNext)),
            ("ap", Item(AddToPlaylist)),
            ("al", SetSaved(true)),
            ("dl", SetSaved(false)),
            ("dp", Item(RemoveFromPlaylist)),
            ("dq", RemoveFromQueue),
        ];
        let t = Instant::now();
        for (keys, command) in table {
            let mut s = Sequencer::default();
            let mut chars = keys.chars();
            let verb = chars.next().unwrap();
            assert_eq!(s.press(verb, t), None, "{keys}");
            let label = format!("{verb}…");
            assert_eq!(s.pending_label(), Some(label.as_str()), "{keys}");
            assert_eq!(s.press(chars.next().unwrap(), t), Some(command), "{keys}");
            assert_eq!(s.pending_label(), None, "{keys}");
        }
    }

    fn test_app() -> App {
        let root = std::env::temp_dir().join(format!(
            "spotifast-winamp-keys-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut app = App::new(
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
        );
        crate::demo::populate(&mut app);
        app
    }

    /// A key as the keyboard sends it: the key, the letter it types, and
    /// the key let go, since egui counts a press of a key still held down
    /// as a repeat.
    fn typed(key: Key, text: &str) -> Vec<Event> {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let mut events = vec![event(true)];
        if !text.is_empty() {
            events.push(Event::Text(text.into()));
        }
        events.push(event(false));
        events
    }

    /// One frame of input in the order the mini player takes it: FIND's
    /// box, then these keys, then the window's own.
    fn frame(app: &mut App, ctx: &egui::Context, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                super::super::find::keys(app, ui.ctx());
                handle(app, ui.ctx());
                crate::ui::keys::handle(app, ui.ctx());
            },
        );
        output.textures_delta.clear();
    }

    /// Two keys pressed one frame after the other.
    fn sequence(app: &mut App, ctx: &egui::Context, first: Key, second: Key) {
        let text = |key: Key| key.name().to_lowercase();
        frame(app, ctx, typed(first, &text(first)));
        frame(app, ctx, typed(second, &text(second)));
    }

    fn playing(app: &App) -> String {
        app.now_playing().expect("the demo plays a song").uri
    }

    #[test]
    fn typing_in_find_never_triggers_shortcuts() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.settings.find_open = true;
        app.winamp.find.focus_requested = true;
        frame(&mut app, &ctx, vec![]);
        assert!(app.winamp.find.focused);
        app.actions.clear();
        let shuffle = app.now_playing().map(|now| now.shuffle);
        for (key, text) in [(Key::G, "g"), (Key::A, "a"), (Key::S, "s"), (Key::M, "m")] {
            frame(&mut app, &ctx, typed(key, text));
        }
        assert_eq!(app.winamp.find.query, "gasm");
        assert!(
            app.actions
                .iter()
                .all(|action| matches!(action, Action::FindSearch(_))),
            "{:?}",
            app.actions
        );
        assert_eq!(app.winamp.keys.pending_label(), None);
        assert_eq!(app.now_playing().map(|now| now.shuffle), shuffle);
        app.backend.shutdown();
    }

    #[test]
    fn the_window_keys_still_reach_the_window() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        for (key, text, wanted) in [
            (Key::Space, " ", "TogglePlay"),
            (Key::M, "m", "ToggleMute"),
            (Key::S, "s", "ToggleShuffle"),
            (Key::R, "r", "CycleRepeat"),
            (Key::B, "b", "ToggleSaved"),
            (Key::Questionmark, "?", "ShowDialog(Shortcuts)"),
        ] {
            app.actions.clear();
            frame(&mut app, &ctx, typed(key, text));
            assert_eq!(app.actions.len(), 1, "{key:?} pushed {:?}", app.actions);
            assert!(
                format!("{:?}", app.actions[0]).starts_with(wanted),
                "{key:?} pushed {:?}",
                app.actions
            );
        }
        app.backend.shutdown();
    }

    #[test]
    fn section_keys_toggle_the_mini_players_own_sections() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        for (key, wanted) in [
            (Key::E, "ToggleWinampEq"),
            (Key::Q, "ToggleWinampPlaylist"),
            (Key::L, "ToggleWinampSection(Lyrics)"),
            (Key::V, "ToggleWinampSection(Vis)"),
        ] {
            app.actions.clear();
            frame(&mut app, &ctx, typed(key, &key.name().to_lowercase()));
            assert_eq!(
                format!("{:?}", app.actions),
                format!("[{wanted}]"),
                "{key:?}, not the big window's panel"
            );
        }
        app.backend.shutdown();
    }

    #[test]
    fn slash_and_command_f_focus_find_opening_it_first() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        assert!(!app.settings.find_open);
        frame(&mut app, &ctx, typed(Key::Slash, "/"));
        assert!(matches!(
            app.actions.as_slice(),
            [Action::ToggleWinampSection(WinampSection::Find)]
        ));
        assert!(app.winamp.find.focus_requested);

        app.actions.clear();
        app.winamp.find.focus_requested = false;
        frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert!(
            !app.actions
                .iter()
                .any(|action| matches!(action, Action::FocusSearch)),
            "the big window's search stays closed: {:?}",
            app.actions
        );
        assert!(app.winamp.find.focus_requested);
        app.backend.shutdown();
    }

    #[test]
    fn a_pending_verb_shows_and_an_unknown_second_key_does_nothing() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        frame(&mut app, &ctx, typed(Key::G, "g"));
        assert_eq!(app.winamp.keys.pending_label(), Some("g…"));
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        // M is not a second key for go: the sequence ends, and nothing mutes.
        frame(&mut app, &ctx, typed(Key::M, "m"));
        assert_eq!(app.winamp.keys.pending_label(), None);
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        // And M is the window's again.
        frame(&mut app, &ctx, typed(Key::M, "m"));
        assert!(matches!(app.actions.as_slice(), [Action::ToggleMute]));
        app.backend.shutdown();
    }

    /// R2: `a l` only likes, `d l` only unlikes; neither toggles back.
    #[test]
    fn like_and_unlike_change_only_what_needs_changing() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let uri = playing(&app);
        let toggled = |app: &App| {
            app.actions
                .iter()
                .any(|action| matches!(action, Action::ToggleSaved(u) if *u == uri))
        };
        app.saved.insert(uri.clone(), false);
        sequence(&mut app, &ctx, Key::D, Key::L);
        assert!(!toggled(&app), "not liked, so nothing to unlike");
        sequence(&mut app, &ctx, Key::A, Key::L);
        assert!(toggled(&app), "{:?}", app.actions);

        app.actions.clear();
        app.saved.insert(uri.clone(), true);
        sequence(&mut app, &ctx, Key::A, Key::L);
        assert!(!toggled(&app), "already liked");
        sequence(&mut app, &ctx, Key::D, Key::L);
        assert!(toggled(&app), "{:?}", app.actions);
        app.backend.shutdown();
    }

    fn copied(app: &App) -> Option<String> {
        app.actions.iter().find_map(|action| match action {
            Action::CopyLink(uri) => Some(uri.clone()),
            _ => None,
        })
    }

    #[test]
    fn item_keys_act_on_the_list_picked_last_or_the_playing_song() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let now = playing(&app);

        // Nothing picked: the playing song.
        frame(&mut app, &ctx, typed(Key::Y, "y"));
        assert_eq!(copied(&app), Some(now.clone()));

        // A row picked in PL.
        app.actions.clear();
        app.settings.playlist_open = true;
        app.winamp.playlist_selection.insert(1);
        app.winamp.find_picked_last = false;
        let queued = app.queue.get().unwrap().queue[0].uri().to_string();
        frame(&mut app, &ctx, typed(Key::Y, "y"));
        assert_eq!(copied(&app), Some(queued.clone()));

        // A row picked in FIND since.
        app.actions.clear();
        app.settings.find_open = true;
        let rows = super::super::find::visible_rows(&app);
        assert!(rows.len() > 2, "the demo's Liked Songs");
        app.winamp.find.selected = Some(2);
        app.winamp.find_picked_last = true;
        frame(&mut app, &ctx, typed(Key::Y, "y"));
        assert_eq!(copied(&app), Some(rows[2].uri.clone()));

        // FIND closed: back to the playing song, not a hidden row.
        app.actions.clear();
        app.settings.find_open = false;
        frame(&mut app, &ctx, typed(Key::Y, "y"));
        assert_eq!(copied(&app), Some(now));
        app.backend.shutdown();
    }

    #[test]
    fn go_history_and_go_library_switch_finds_list() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.settings.find_open = true;
        app.winamp.find.set_query("Bonobo".into());
        sequence(&mut app, &ctx, Key::G, Key::H);
        assert_eq!(
            app.winamp.find.mode,
            super::super::find::state::FindMode::Recent
        );
        assert!(app.winamp.find.query.is_empty());

        app.winamp
            .find
            .set_mode(super::super::find::state::FindMode::Albums);
        app.winamp
            .find
            .set_mode(super::super::find::state::FindMode::Recent);
        sequence(&mut app, &ctx, Key::G, Key::Y);
        assert_eq!(
            app.winamp.find.mode,
            super::super::find::state::FindMode::Albums,
            "the library kind last listed"
        );
        app.backend.shutdown();
    }

    #[test]
    fn go_devices_asks_for_the_devices_menu() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        assert!(!app.winamp.open_devices);
        sequence(&mut app, &ctx, Key::G, Key::D);
        assert!(app.winamp.open_devices);
        app.backend.shutdown();
    }

    #[test]
    fn add_to_playlist_opens_the_picker_for_the_song() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let now = playing(&app);
        sequence(&mut app, &ctx, Key::A, Key::P);
        assert_eq!(
            app.winamp
                .playlist_picker_for
                .as_ref()
                .map(|row| row.uri.clone()),
            Some(now)
        );
        app.backend.shutdown();
    }

    #[test]
    fn remove_from_queue_takes_the_picked_queued_song() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let now = app.now_playing();
        app.local_ready = true;
        app.local.track = Some(crate::player::LocalTrack {
            uri: now.as_ref().unwrap().uri.clone(),
            ..Default::default()
        });
        app.local.playback = crate::player::Playback::Paused;
        let queued: Vec<String> = app.queue.get().unwrap().queue[..2]
            .iter()
            .map(|item| item.uri().to_string())
            .collect();
        app.manual_queue = queued.clone();
        app.settings.playlist_open = true;
        // The playing song is row 0; row 2 is the second queued song.
        app.winamp.playlist_selection.insert(2);
        sequence(&mut app, &ctx, Key::D, Key::Q);
        assert!(
            matches!(
                app.actions.as_slice(),
                [Action::RemoveFromQueue { rows }] if *rows == [(1, queued[1].clone())]
            ),
            "{:?}",
            app.actions
        );
        assert!(app.winamp.playlist_selection.is_empty());
        app.backend.shutdown();
    }

    #[test]
    fn go_playing_opens_what_it_plays_from_in_find_and_picks_it() {
        use super::super::find::state::FindPage;
        let ctx = egui::Context::default();
        let mut app = test_app();
        let now = app.now_playing().unwrap();
        assert_eq!(
            app.playing_context_uri().as_deref(),
            Some("spotify:playlist:pl1")
        );
        sequence(&mut app, &ctx, Key::G, Key::P);
        assert!(
            app.actions
                .iter()
                .any(|action| matches!(action, Action::ToggleWinampSection(WinampSection::Find))),
            "{:?}",
            app.actions
        );
        assert!(
            matches!(app.winamp.find.page(), FindPage::Playlist { id, .. } if id == "pl1"),
            "{:?}",
            app.winamp.find.page()
        );
        assert_eq!(app.winamp.find.reselect.as_deref(), Some(now.uri.as_str()));

        // Nothing to play from: the song's album.
        if let Some(remote) = app.remote.as_mut() {
            remote.state.context = None;
        }
        assert_eq!(app.playing_context_uri(), None);
        assert!(app.now_playing().is_some());
        let album = now.album_id.clone().expect("the demo song's album");
        sequence(&mut app, &ctx, Key::G, Key::P);
        assert!(
            matches!(app.winamp.find.page(), FindPage::Album { id, .. } if *id == album),
            "{:?}",
            app.winamp.find.page()
        );
        app.backend.shutdown();
    }

    /// R15: Esc in the box lets go and keeps the query; then it empties
    /// the box; then it closes FIND. The window's Esc never sees those,
    /// and has it back once FIND is closed.
    #[test]
    fn escape_lets_go_then_empties_the_box_then_closes_find() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.settings.find_open = true;
        app.winamp.find.focus_requested = true;
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, typed(Key::X, "x"));
        assert_eq!(app.winamp.find.query, "x");
        // The window's Esc would close the devices list.
        app.show_devices = true;
        app.actions.clear();

        frame(&mut app, &ctx, typed(Key::Escape, ""));
        assert!(!app.winamp.find.focused, "the first Esc lets go");
        assert_eq!(app.winamp.find.query, "x", "and keeps the query");
        assert!(app.actions.is_empty(), "{:?}", app.actions);

        frame(&mut app, &ctx, typed(Key::Escape, ""));
        assert!(app.winamp.find.query.is_empty(), "the second empties it");
        assert!(
            !app.actions
                .iter()
                .any(|action| matches!(action, Action::ToggleWinampSection(_))),
            "{:?}",
            app.actions
        );

        frame(&mut app, &ctx, typed(Key::Escape, ""));
        assert!(
            matches!(
                app.actions.last(),
                Some(Action::ToggleWinampSection(WinampSection::Find))
            ),
            "the third closes FIND: {:?}",
            app.actions
        );
        assert!(app.show_devices, "FIND's Esc is not the window's too");

        // FIND closed: Esc is the window's again.
        app.settings.find_open = false;
        app.actions.clear();
        frame(&mut app, &ctx, typed(Key::Escape, ""));
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        assert!(!app.show_devices);
        app.backend.shutdown();
    }

    fn queued(app: &App) -> Vec<String> {
        app.actions
            .iter()
            .filter_map(|action| match action {
                Action::AddToQueue { uri, .. } => Some(uri.clone()),
                _ => None,
            })
            .collect()
    }

    /// Without the mouse: type a search, Esc out of the box, and `a q`
    /// queues the first result, or the one the arrows picked.
    #[test]
    fn a_search_typed_then_escaped_is_what_the_keys_act_on() {
        let ctx = egui::Context::default();
        for downs in [0usize, 2] {
            let mut app = test_app();
            app.settings.find_open = true;
            app.winamp.find.focus_requested = true;
            frame(&mut app, &ctx, vec![]);
            // The demo's search for "Bonobo" is answered.
            frame(&mut app, &ctx, vec![Event::Text("Bonobo".into())]);
            let rows = super::super::find::visible_rows(&app);
            assert!(rows.len() > 2, "the demo's answer");
            for _ in 0..downs {
                frame(&mut app, &ctx, typed(Key::ArrowDown, ""));
            }
            frame(&mut app, &ctx, typed(Key::Escape, ""));
            assert!(!app.winamp.find.focused);
            assert_eq!(app.winamp.find.query, "Bonobo");
            app.actions.clear();
            sequence(&mut app, &ctx, Key::A, Key::Q);
            let picked = downs.saturating_sub(1);
            assert_eq!(queued(&app), [rows[picked].uri.clone()], "{downs} downs");
            app.backend.shutdown();
        }
    }

    /// R16: a key that is not a letter ends a waiting verb and does its
    /// own job.
    #[test]
    fn a_key_that_is_not_a_letter_ends_the_verb_and_does_its_job() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        frame(&mut app, &ctx, typed(Key::G, "g"));
        assert_eq!(app.winamp.keys.pending_label(), Some("g…"));
        frame(&mut app, &ctx, typed(Key::Space, " "));
        assert_eq!(app.winamp.keys.pending_label(), None);
        assert!(
            matches!(app.actions.as_slice(), [Action::TogglePlay]),
            "{:?}",
            app.actions
        );

        app.actions.clear();
        frame(&mut app, &ctx, typed(Key::A, "a"));
        frame(&mut app, &ctx, typed(Key::Slash, "/"));
        assert_eq!(app.winamp.keys.pending_label(), None);
        assert!(app.winamp.find.focus_requested, "/ focuses FIND");
        app.backend.shutdown();
    }

    /// A verb waiting when FIND's box takes the keyboard never turns what
    /// is typed next into a command.
    #[test]
    fn a_letter_typed_into_the_box_never_ends_a_waiting_verb() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.settings.find_open = true;
        frame(&mut app, &ctx, typed(Key::G, "g"));
        assert_eq!(app.winamp.keys.pending_label(), Some("g…"));
        frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert!(app.winamp.find.focus_requested);
        frame(&mut app, &ctx, typed(Key::A, "a"));
        assert!(app.winamp.find.focused);
        assert_eq!(app.winamp.find.query, "a");
        assert_eq!(app.winamp.keys.pending_label(), None);
        assert!(app.actions.is_empty(), "no g a: {:?}", app.actions);
        assert_eq!(
            app.winamp.find.page(),
            &super::super::find::state::FindPage::Top
        );
        app.backend.shutdown();
    }
}
