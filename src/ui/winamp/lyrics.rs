//! LYRICS: the playing song's words under the playlist, one line to a
//! row and centred, the line being sung in the list's "current" colour.
//!
//! Like the lyrics panel, the list follows the song until the reader
//! scrolls it, and a click on a timed line plays from there and follows
//! again. A track change follows the new song (`App::request_lyrics`).

use std::borrow::Cow;
use std::time::Duration;

use crate::app::{App, NowPlaying};
use crate::i18n::{Locale, gettext};
use crate::lyrics::Lyrics;
use crate::model::{Action, Loadable, WinampSection};
use crate::skin::layout;

use super::View;
use super::section::{self, ListRow};

/// What the section has to say: the words, or one line in their place.
enum Body<'a> {
    Words(&'a Lyrics),
    /// Loading, no lyrics, instrumental, or, `failed`, why the words could
    /// not be fetched, which a click asks for again.
    Note {
        text: String,
        failed: bool,
    },
}

fn note(text: impl Into<String>) -> Body<'static> {
    Body::Note {
        text: text.into(),
        failed: false,
    }
}

/// The words for `now`, or the line that stands in for them. `uri` is the
/// song `lyrics` are for.
fn body<'a>(
    lyrics: &'a Loadable<Option<Lyrics>>,
    uri: Option<&str>,
    locale: Locale,
    now: &NowPlaying,
) -> Body<'a> {
    if uri != Some(now.uri.as_str()) {
        return note(gettext(locale, "…"));
    }
    match lyrics {
        Loadable::NotLoaded | Loadable::Loading => note(gettext(locale, "…")),
        Loadable::Loaded(None) => note(gettext(locale, "No lyrics")),
        Loadable::Loaded(Some(lyrics)) if lyrics.instrumental => {
            note(gettext(locale, "Instrumental"))
        }
        Loadable::Loaded(Some(lyrics)) => Body::Words(lyrics),
        Loadable::Failed(error) => Body::Note {
            text: gettext(
                locale,
                // Translators: Keep {error} unchanged. It is the original failure detail.
                "Couldn't fetch the lyrics: {error}",
            )
            .replace("{error}", error),
            failed: true,
        },
    }
}

/// The section, under the sections above it. `focused` is the window's.
pub(super) fn show(app: &mut App, view: &mut View, now: Option<&NowPlaying>, focused: bool) {
    let Some(chrome) = section::frame(app, view, WinampSection::Lyrics, "LYRICS", focused) else {
        return;
    };
    // With nothing playing the body stays empty, as the playlist's does.
    let Some(now) = now else {
        return;
    };
    let area = section::list_area(chrome.height);
    let visible = (area.height / layout::PLAYLIST_TRACK_HEIGHT) as usize;
    let following = app.lyrics_following;
    let body = body(&app.lyrics, app.lyrics_uri.as_deref(), app.locale, now);
    let winamp = &mut app.winamp;

    let lyrics = match body {
        Body::Words(lyrics) => lyrics,
        Body::Note { text, failed } => {
            winamp.lyrics_scroll = 0;
            let response = section::list(
                view,
                &mut winamp.playlist_text,
                1,
                |_| ListRow {
                    label: Cow::Borrowed(text.as_str()),
                    right: None,
                    // The error stands out as FIND's does: it is the
                    // line to click.
                    current: failed,
                    selected: false,
                    centred: true,
                },
                &mut winamp.lyrics_scroll,
                &mut winamp.lyrics_wheel,
                area,
                WinampSection::Lyrics,
            );
            if failed && response.clicked.is_some() {
                app.actions.push(Action::RetryLyrics);
            }
            return;
        }
    };

    let active = lyrics.active_line(now.position_ms);
    // Words without timing can only be followed by the clock: keep the
    // part of the text the song is probably at in the middle.
    let followed = if lyrics.synced {
        active
    } else if now.duration_ms > 0 && !lyrics.lines.is_empty() {
        let fraction = (f64::from(now.position_ms) / f64::from(now.duration_ms)).clamp(0.0, 1.0);
        Some(((lyrics.lines.len() - 1) as f64 * fraction).round() as usize)
    } else {
        None
    };
    if following {
        winamp.lyrics_scroll = first_visible(followed, visible, lyrics.lines.len());
    }
    let before = winamp.lyrics_scroll;
    let response = section::list(
        view,
        &mut winamp.playlist_text,
        lyrics.lines.len(),
        |index| {
            let line = &lyrics.lines[index];
            // A timed line with no words is the band playing on.
            let text = if line.text.is_empty() && lyrics.synced {
                "\u{266a}"
            } else {
                line.text.as_str()
            };
            ListRow {
                label: Cow::Borrowed(text),
                right: None,
                current: active == Some(index),
                selected: false,
                centred: true,
            }
        },
        &mut winamp.lyrics_scroll,
        &mut winamp.lyrics_wheel,
        area,
        WinampSection::Lyrics,
    );
    let scrolled = winamp.lyrics_scroll != before;
    let seek = response
        .clicked
        .filter(|_| lyrics.synced)
        .and_then(|index| lyrics.lines.get(index))
        .and_then(|line| line.at_ms);
    let next = lyrics
        .lines
        .iter()
        .filter_map(|line| line.at_ms)
        .find(|at| *at > now.position_ms)
        .filter(|_| now.playing && lyrics.synced);

    // Scrolling by hand means the reader wants to look elsewhere; a click
    // on a line, or the next song, picks the song back up.
    if scrolled && following {
        app.actions.push(Action::PauseLyricsFollow);
    }
    if let Some(at_ms) = seek {
        app.actions.push(Action::Seek(at_ms));
        app.actions.push(Action::FollowLyrics);
    }
    if let Some(next) = next {
        view.ui
            .ctx()
            .request_repaint_after(Duration::from_millis(u64::from(next - now.position_ms)));
    }
}

/// The first line in view that puts `active` in the middle of `visible`
/// rows, kept from running past either end of `len` lines.
pub(super) fn first_visible(active: Option<usize>, visible: usize, len: usize) -> usize {
    active
        .unwrap_or(0)
        .saturating_sub(visible / 2)
        .min(len.saturating_sub(visible))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_active_line_sits_in_the_middle_until_the_ends() {
        assert_eq!(first_visible(Some(0), 5, 40), 0);
        assert_eq!(first_visible(Some(10), 5, 40), 8);
        assert_eq!(first_visible(Some(39), 5, 40), 35);
        assert_eq!(first_visible(None, 5, 3), 0);
    }

    use egui::Event;

    /// The demo, its song paused 52 s in, with LYRICS open eight rows
    /// tall over 40 lines five seconds apart: line 10 is being sung.
    fn lyrics_app() -> App {
        let root = std::env::temp_dir().join(format!(
            "spotifast-lyrics-section-{}-{}",
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
        app.settings.lyrics_open = true;
        app.settings.lyrics_height = 174;
        if let Some(remote) = &mut app.remote {
            remote.state.is_playing = false;
            remote.state.progress_ms = Some(52_000);
        }
        app.lyrics_uri = app.now_playing().map(|now| now.uri);
        app.lyrics = Loadable::Loaded(Some(Lyrics {
            lines: (0..40)
                .map(|i| crate::lyrics::Line {
                    at_ms: Some(i * 5000),
                    text: format!("Line {i}"),
                })
                .collect(),
            synced: true,
            instrumental: false,
        }));
        app.lyrics_following = true;
        app
    }

    /// Draws LYRICS once into a context at one skin pixel to the point.
    fn draw(app: &mut App, ctx: &egui::Context, events: Vec<Event>) {
        let skin = app.winamp.skin.clone();
        let textures = std::collections::HashMap::new();
        let now = app.now_playing();
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
                show(app, &mut view, now.as_ref(), true);
            },
        );
        output.textures_delta.clear();
    }

    /// The middle of the `row`th row in view.
    fn row_at(app: &App, row: u32) -> egui::Pos2 {
        let area = section::list_area(app.settings.lyrics_height);
        egui::pos2(
            (area.x + area.width / 2) as f32,
            (area.y + row * layout::PLAYLIST_TRACK_HEIGHT + 6) as f32,
        )
    }

    fn click(app: &mut App, ctx: &egui::Context, pos: egui::Pos2) {
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

    #[test]
    fn following_keeps_the_sung_line_in_the_middle() {
        let ctx = egui::Context::default();
        let mut app = lyrics_app();
        draw(&mut app, &ctx, vec![]);
        assert_eq!(app.winamp.lyrics_scroll, 6, "line 10 of eight rows in view");
        assert!(app.actions.is_empty());
        app.backend.shutdown();
    }

    #[test]
    fn the_wheel_stops_following_and_scrolls_freely() {
        let ctx = egui::Context::default();
        let mut app = lyrics_app();
        let pos = row_at(&app, 2);
        draw(&mut app, &ctx, vec![Event::PointerMoved(pos)]);
        draw(
            &mut app,
            &ctx,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -1.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            }],
        );
        assert_eq!(app.winamp.lyrics_scroll, 9, "a notch is three rows down");
        assert!(matches!(
            app.actions.as_slice(),
            [Action::PauseLyricsFollow]
        ));
        app.actions.clear();
        app.lyrics_following = false;
        draw(&mut app, &ctx, vec![]);
        assert_eq!(
            app.winamp.lyrics_scroll, 9,
            "the list stays where it was put"
        );
        assert!(app.actions.is_empty());
        app.backend.shutdown();
    }

    #[test]
    fn a_click_on_a_line_plays_from_it_and_follows() {
        let ctx = egui::Context::default();
        let mut app = lyrics_app();
        app.lyrics_following = false;
        app.winamp.lyrics_scroll = 20;
        let pos = row_at(&app, 1);
        click(&mut app, &ctx, pos);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::Seek(105_000), Action::FollowLyrics]
        ));
        app.backend.shutdown();
    }

    #[test]
    fn a_click_on_the_error_asks_again() {
        let ctx = egui::Context::default();
        let mut app = lyrics_app();
        app.lyrics = Loadable::Failed("offline".into());
        let pos = row_at(&app, 0);
        click(&mut app, &ctx, pos);
        assert!(matches!(app.actions.as_slice(), [Action::RetryLyrics]));
        app.actions.clear();
        app.lyrics = Loadable::Loaded(None);
        let pos = row_at(&app, 0);
        click(&mut app, &ctx, pos);
        assert!(app.actions.is_empty(), "No lyrics is only a note");
        app.backend.shutdown();
    }
}
