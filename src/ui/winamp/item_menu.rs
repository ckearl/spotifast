//! What can be done to a song, artist, album or playlist from the mini
//! player: one set of actions, whichever row it is on. A right-click in
//! FIND or PL opens them as a menu, FIND's +LIST and RADIO reach two of
//! them, and the keyboard reaches them through [`perform`].
//!
//! Nothing here leaves the mini player: going to an artist, an album or a
//! radio opens it inside FIND, opening FIND first if it is closed.

use std::borrow::Cow;

use egui::Ui;

use crate::api::models::{Album, ArtistRef, PlayableItem, Track};
use crate::app::App;
use crate::i18n::{gettext, pgettext};
use crate::model::{Action, Loadable, RowContext, WinampSection};
use crate::util;

use super::View;
use super::find::state::{Choice, FindPage, FindRow, ItemKind};

/// One thing to do to a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ItemAction {
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
    ToggleSaved,
    RemoveFromPlaylist,
    GoToArtist,
    GoToAlbum,
    Radio,
    CopyLink,
}

use ItemAction::*;

const SONG: [ItemAction; 9] = [
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
    ToggleSaved,
    GoToArtist,
    GoToAlbum,
    Radio,
    CopyLink,
];
/// A song inside a playlist the account may edit.
const OWN_SONG: [ItemAction; 10] = [
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
    ToggleSaved,
    RemoveFromPlaylist,
    GoToArtist,
    GoToAlbum,
    Radio,
    CopyLink,
];
const ARTIST: [ItemAction; 4] = [Play, ToggleSaved, Radio, CopyLink];
/// An album or a playlist.
const COLLECTION: [ItemAction; 5] = [Play, AddToQueue, ToggleSaved, Radio, CopyLink];

/// The actions a kind of row offers, in the menu's order.
pub(super) fn actions_for(kind: ItemKind, in_own_playlist: bool) -> &'static [ItemAction] {
    match kind {
        ItemKind::Song if in_own_playlist => &OWN_SONG,
        ItemKind::Song => &SONG,
        ItemKind::Artist => &ARTIST,
        ItemKind::Album | ItemKind::Playlist => &COLLECTION,
    }
}

/// The playlist a row sits in, when the account may edit it.
fn own_playlist(context: Option<&RowContext>) -> Option<&str> {
    match context? {
        RowContext::Context {
            editable_playlist: Some((id, _)),
            ..
        }
        | RowContext::View {
            editable_playlist: Some((id, _)),
            ..
        } => Some(id),
        _ => None,
    }
}

/// Whether the row sits in a playlist the account may edit.
pub(super) fn in_own_playlist(context: Option<&RowContext>) -> bool {
    own_playlist(context).is_some()
}

/// The row's own name: its label may lead with its artists.
fn own_name(row: &FindRow) -> &str {
    if row.name.is_empty() {
        &row.label
    } else {
        &row.name
    }
}

/// The artists a song's label leads with, "Who" of "Who - what".
fn label_artists(row: &FindRow) -> Option<&str> {
    row.label
        .strip_suffix(row.name.as_str())?
        .strip_suffix(" - ")
        .filter(|who| !who.is_empty())
}

/// The song or episode `uri` as the app holds it, wherever that is: the
/// lists FIND and PL draw from.
fn held(app: &App, uri: &str) -> Option<PlayableItem> {
    let track = |track: &Track| (track.uri == uri).then(|| PlayableItem::Track(track.clone()));
    let item = |item: &PlayableItem| (item.uri() == uri).then(|| item.clone());
    if let Some(found) = util::uri_id(uri)
        .and_then(|id| app.track_cache.get(id))
        .and_then(track)
    {
        return Some(found);
    }
    if let Loadable::Loaded(queue) = &app.queue
        && let Some(found) = queue
            .currently_playing
            .iter()
            .chain(&queue.queue)
            .find_map(item)
    {
        return Some(found);
    }
    let tracks = |loadable: &Loadable<Vec<Track>>| match loadable {
        Loadable::Loaded(tracks) => tracks.iter().find_map(track),
        _ => None,
    };
    app.library
        .liked
        .items
        .iter()
        .find_map(|saved| track(&saved.track))
        .or_else(|| {
            app.album_pages
                .values()
                .find_map(|page| page.tracks.items.iter().find_map(track))
        })
        .or_else(|| {
            app.playlist_pages.values().find_map(|page| {
                page.items
                    .items
                    .iter()
                    .find_map(|entry| entry.playable().and_then(item))
            })
        })
        .or_else(|| {
            app.artist_pages
                .values()
                .find_map(|page| tracks(&page.top_tracks))
        })
        .or_else(|| {
            app.radio_pages
                .values()
                .find_map(|page| tracks(&page.songs))
        })
        .or_else(|| match &app.search.results {
            Loadable::Loaded(results) => results
                .tracks
                .as_ref()
                .and_then(|page| page.items.iter().find_map(track)),
            _ => None,
        })
        .or_else(|| match &app.home.recently_played {
            Loadable::Loaded(played) => played.iter().find_map(|played| track(&played.track)),
            _ => None,
        })
}

/// The song a song row stands for, to queue or add to a playlist: as the
/// app holds it, or sketched from the row, which has its name, length,
/// artists and album. `None` for anything that is not a song, and for an
/// episode the app no longer holds.
pub(super) fn playable(app: &App, row: &FindRow) -> Option<PlayableItem> {
    if row.kind != ItemKind::Song || row.uri.is_empty() {
        return None;
    }
    if let Some(found) = held(app, &row.uri) {
        return Some(found);
    }
    if util::uri_kind(&row.uri) != Some("track") {
        return None;
    }
    let id = |uri: &str| util::uri_id(uri).unwrap_or_default().to_string();
    Some(PlayableItem::Track(Track {
        id: Some(id(&row.uri)),
        name: own_name(row).to_string(),
        uri: row.uri.clone(),
        duration_ms: row.duration_ms.unwrap_or_default(),
        artists: label_artists(row)
            .map(|who| ArtistRef {
                id: row.artist_uri.as_deref().map(id),
                name: who.to_string(),
                uri: row.artist_uri.clone(),
            })
            .into_iter()
            .collect(),
        album: row.album_uri.as_deref().map(|uri| Album {
            id: id(uri),
            uri: uri.to_string(),
            ..Album::default()
        }),
        ..Track::default()
    }))
}

/// The artist and album a row goes to: itself when it is one.
fn artist_uri(row: &FindRow) -> Option<&str> {
    match row.kind {
        ItemKind::Artist => Some(&row.uri),
        ItemKind::Song => row.artist_uri.as_deref(),
        _ => None,
    }
}

fn album_uri(row: &FindRow) -> Option<&str> {
    match row.kind {
        ItemKind::Album => Some(&row.uri),
        ItemKind::Song => row.album_uri.as_deref(),
        _ => None,
    }
}

/// The page FIND opens for going to the row's artist.
fn artist_page(app: &App, row: &FindRow) -> Option<FindPage> {
    let uri = artist_uri(row)?;
    let id = util::uri_id(uri)?.to_string();
    let name = app
        .artist_pages
        .get(&id)
        .and_then(|page| page.artist.get())
        .map(|artist| artist.name.clone())
        .or_else(|| match held(app, &row.uri) {
            Some(PlayableItem::Track(track)) => track
                .artists
                .iter()
                .find(|artist| artist.uri.as_deref() == Some(uri))
                .map(|artist| artist.name.clone()),
            _ => None,
        })
        .or_else(|| match row.kind {
            ItemKind::Artist => Some(own_name(row).to_string()),
            _ => label_artists(row).map(str::to_string),
        })
        .unwrap_or_else(|| gettext(app.locale, "Artist").into_owned());
    Some(FindPage::Artist {
        id,
        name,
        albums: false,
    })
}

/// The page FIND opens for going to the row's album.
fn album_page(app: &App, row: &FindRow) -> Option<FindPage> {
    let uri = album_uri(row)?;
    let id = util::uri_id(uri)?.to_string();
    let name = app
        .album_pages
        .get(&id)
        .and_then(|page| page.album.get())
        .map(|album| album.name.clone())
        .or_else(|| match row.kind {
            ItemKind::Album => Some(own_name(row).to_string()),
            _ => match held(app, &row.uri) {
                Some(PlayableItem::Track(track)) => track.album.map(|album| album.name),
                _ => None,
            },
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| gettext(app.locale, "Album").into_owned());
    Some(FindPage::Album { id, name })
}

/// The radio seeded by the row, as a page of FIND.
fn radio_page(app: &App, row: &FindRow) -> Option<FindPage> {
    util::station_uri(&row.uri)?;
    Some(FindPage::Radio {
        seed: row.uri.clone(),
        name: gettext(app.locale, "{track} Radio").replace("{track}", own_name(row)),
    })
}

/// Whether the row is a playlist the account owns. Unfollowing one's own
/// playlist deletes it, which the big window asks to confirm, so it is
/// not offered here.
fn owned_playlist(app: &App, row: &FindRow) -> bool {
    if row.kind != ItemKind::Playlist {
        return false;
    }
    let (Some(user), Some(id)) = (app.user_id(), util::uri_id(&row.uri)) else {
        return false;
    };
    app.library
        .playlists
        .get()
        .and_then(|lists| lists.iter().find(|list| list.id == id))
        .or_else(|| app.playlist_pages.get(id)?.playlist.get())
        .is_some_and(|list| list.owned_by(user))
}

/// Whether an action can be done to the row now.
fn available(app: &App, row: &FindRow, action: ItemAction) -> bool {
    match action {
        Play => !row.uri.is_empty(),
        ToggleSaved => !row.uri.is_empty() && !owned_playlist(app, row),
        PlayNext => {
            app.queue_locally_reorderable()
                && app.can_queue(&row.uri)
                && playable(app, row).is_some()
        }
        AddToQueue => match row.kind {
            ItemKind::Song => app.can_queue(&row.uri),
            ItemKind::Album | ItemKind::Playlist => true,
            ItemKind::Artist => false,
        },
        AddToPlaylist => playable(app, row).is_some(),
        RemoveFromPlaylist => row.kind == ItemKind::Song,
        GoToArtist => artist_uri(row).is_some(),
        GoToAlbum => album_uri(row).is_some(),
        Radio => util::station_uri(&row.uri).is_some(),
        CopyLink => util::open_spotify_url(&row.uri).is_some(),
    }
}

/// The menu's words for an action on the row. Saving says what it does to
/// this kind: a song is liked, an artist followed, an album or playlist
/// saved to the library; and once it is, the words undo it.
pub(super) fn label(app: &App, row: &FindRow, action: ItemAction) -> Cow<'static, str> {
    let locale = app.locale;
    match action {
        Play => gettext(locale, "Play"),
        PlayNext => gettext(locale, "Play next"),
        AddToQueue => gettext(locale, "Add to queue"),
        AddToPlaylist => gettext(locale, "Add to playlist"),
        ToggleSaved => {
            let saved = app.is_saved(&row.uri) == Some(true);
            match (row.kind, saved) {
                // Translators: Takes a song out of Liked Songs.
                (ItemKind::Song, true) => pgettext(locale, "song", "Unlike"),
                // Translators: Puts a song in Liked Songs.
                (ItemKind::Song, false) => pgettext(locale, "song", "Like"),
                (ItemKind::Artist, true) => pgettext(locale, "artist", "Unfollow"),
                (ItemKind::Artist, false) => pgettext(locale, "artist", "Follow"),
                (_, true) => gettext(locale, "Remove from Your Library"),
                (_, false) => gettext(locale, "Add to Your Library"),
            }
        }
        RemoveFromPlaylist => gettext(locale, "Remove from this playlist"),
        GoToArtist => gettext(locale, "Go to artist"),
        GoToAlbum => gettext(locale, "Go to album"),
        Radio => gettext(locale, "Radio"),
        CopyLink => gettext(locale, "Copy link"),
    }
}

/// Opens `page` inside FIND, opening and rolling down FIND when it is
/// not shown. BACK returns to where FIND was, on the row it came from.
fn open_in_find(app: &mut App, page: FindPage, from: &FindRow) {
    if !app.settings.find_open {
        app.actions
            .push(Action::ToggleWinampSection(WinampSection::Find));
    }
    if app.settings.section_shaded(WinampSection::Find) {
        app.actions
            .push(Action::ToggleWinampSectionShade(WinampSection::Find));
    }
    if app.winamp.find.page() != &page {
        app.winamp.find.open_from(page, Some(from.uri.clone()));
        app.winamp.find_scroll = 0;
    }
}

/// Play on a song plays it as its own list does: PL's queue from that
/// row, or the playing song again from its start; FIND's rows as
/// double-click and PLAY play them. False when the song is in neither,
/// for [`perform`] to play it from its context alone.
fn play_where_listed(app: &mut App, row: &FindRow, context: Option<&RowContext>) -> bool {
    if matches!(context, Some(RowContext::Queue)) {
        match row.position {
            Some(index) => app.actions.push(Action::PlayFromRow {
                context: RowContext::Queue,
                uri: row.uri.clone(),
                index,
            }),
            // The playing row: it starts over, where it is.
            None => {
                if app.now_playing().is_some_and(|now| now.uri == row.uri) {
                    app.actions.push(Action::Seek(0));
                }
            }
        }
        return true;
    }
    let rows = super::find::visible_rows(app);
    let Some(index) = rows.iter().position(|shown| shown == row) else {
        return false;
    };
    match app.winamp.find.choose(row, &rows, index) {
        Choice::Play(action) => {
            app.actions.push(action);
            true
        }
        Choice::Open(_) => false,
    }
}

/// Does `action` to `row`, which sits in `context` when it is one of a
/// list's songs. Anything Spotify hears of goes as an `Action`; the pages
/// it goes to open inside FIND.
pub(super) fn perform(
    app: &mut App,
    row: &FindRow,
    action: ItemAction,
    context: Option<&RowContext>,
) {
    if !available(app, row, action) {
        return;
    }
    match action {
        Play => {
            if row.kind == ItemKind::Song && play_where_listed(app, row, context) {
                return;
            }
            let list = match (row.kind, context) {
                (ItemKind::Song, Some(RowContext::Context { uri, .. })) => Some(uri.clone()),
                (
                    ItemKind::Song,
                    Some(RowContext::View {
                        context_uri: uri, ..
                    }),
                ) => Some(uri.clone()),
                (ItemKind::Song, _) => row.context.clone(),
                _ => None,
            };
            app.actions.push(match (row.kind, list) {
                (ItemKind::Song, Some(uri)) => Action::PlayContext {
                    uri,
                    offset_uri: Some(row.uri.clone()),
                    offset_index: None,
                },
                (ItemKind::Song, None) => Action::PlayUris {
                    uris: vec![row.uri.clone()],
                    index: 0,
                },
                _ => Action::PlayContext {
                    uri: row.uri.clone(),
                    offset_uri: None,
                    offset_index: None,
                },
            });
        }
        PlayNext => {
            if let Some(item) = playable(app, row) {
                app.actions.push(Action::PlayNext { items: vec![item] });
            }
        }
        // A playlist's songs are queued once its pages have all come.
        AddToQueue => app.actions.push(Action::AddToQueue {
            uri: row.uri.clone(),
            label: match row.kind {
                ItemKind::Song => row.label.clone(),
                _ => own_name(row).to_string(),
            },
        }),
        // A menu of its own: the menu and +LIST draw the playlists.
        AddToPlaylist => {}
        ToggleSaved => app.actions.push(Action::ToggleSaved(row.uri.clone())),
        RemoveFromPlaylist => {
            if let Some(id) = own_playlist(context) {
                app.actions.push(Action::RemoveFromPlaylist {
                    playlist_id: id.to_string(),
                    uris: vec![row.uri.clone()],
                    // Only this copy of a song the list holds twice.
                    positions: row.position.into_iter().collect(),
                });
            }
        }
        GoToArtist => {
            if let Some(page) = artist_page(app, row) {
                open_in_find(app, page, row);
            }
        }
        GoToAlbum => {
            if let Some(page) = album_page(app, row) {
                open_in_find(app, page, row);
            }
        }
        Radio => {
            if let Some(page) = radio_page(app, row) {
                open_in_find(app, page, row);
            }
        }
        CopyLink => app.actions.push(Action::CopyLink(row.uri.clone())),
    }
}

/// The menu's entries for `row`, drawn into a menu made by
/// [`super::menu`]. A choice is done at once through [`perform`].
pub(super) fn menu(app: &mut App, ui: &mut Ui, row: &FindRow, context: Option<&RowContext>) {
    let font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Button)
        .map_or(10.0, |font| font.size);
    ui.set_min_width(font * 11.0);
    for &action in actions_for(row.kind, in_own_playlist(context)) {
        // Not offered rather than greyed out: deleting is the big window's.
        if action == ToggleSaved && owned_playlist(app, row) {
            continue;
        }
        if matches!(action, GoToArtist | CopyLink)
            || (action == Radio && row.kind != ItemKind::Song)
        {
            ui.separator();
        }
        let enabled = available(app, row, action);
        let text = label(app, row, action);
        if action == AddToPlaylist
            && let Some(item) = playable(app, row)
        {
            // The big window's add-to-playlist picker, in a submenu of
            // the skin's: a click in it types or scrolls, and choosing a
            // playlist closes the whole menu.
            let query = ui.make_persistent_id("item-menu-playlists");
            let config = egui::menu::MenuConfig::find(ui)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            egui::menu::SubMenuButton::new(text.as_ref())
                .config(config)
                .ui(ui, |ui| pick_playlist(app, ui, &item, query));
            continue;
        }
        if ui
            .add_enabled(enabled, egui::Button::new(text.as_ref()))
            .clicked()
        {
            perform(app, row, action, context);
        }
    }
}

/// The playlists to add `item` to, the filter kept under `query`; the
/// menu holding them closes once one is chosen.
fn pick_playlist(app: &mut App, ui: &mut Ui, item: &PlayableItem, query: egui::Id) {
    let before = app.actions.len();
    // No "New playlist": its dialog is drawn only in the big window.
    crate::ui::widgets::kept_playlist_picker(ui, app, std::slice::from_ref(item), query, false);
    if app.actions.len() > before {
        ui.close();
    }
}

/// The row a menu was opened on, and the list it sits in.
type Target = (FindRow, Option<RowContext>);

/// A menu of the mini player's style at the pointer, under `id`, opened
/// when `opened` brings the row a right-click landed on this frame. The
/// row is kept with the menu, so rows that move or go while it is open
/// (an unliked song leaving Liked Songs) never change what it acts on.
fn popup<T: Clone + Send + Sync + 'static>(
    view: &mut View,
    id: egui::Id,
    opened: Option<T>,
) -> Option<(egui::Popup<'static>, T)> {
    let ctx = view.ui.ctx().clone();
    let open = opened.is_some();
    if let Some(target) = opened {
        ctx.data_mut(|data| data.insert_temp(id, target));
    } else if !egui::Popup::is_id_open(&ctx, id) {
        return None;
    }
    let target = ctx.data(|data| data.get_temp::<T>(id))?;
    let popup = egui::Popup::new(id, ctx, egui::PopupAnchor::PointerFixed, view.ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)));
    Some((popup, target))
}

/// The right-click menu of a list, `id` naming which; `opened` is the row
/// right-clicked this frame and the list it sits in.
pub(super) fn context_menu(app: &mut App, view: &mut View, id: &str, opened: Option<Target>) {
    let Some((popup, (row, context))) = popup(view, egui::Id::new(("item-menu", id)), opened)
    else {
        return;
    };
    super::menu(popup, view.skin, view.unit, |ui| {
        menu(app, ui, &row, context.as_ref());
    });
}

/// +LIST: your playlists, filtered as you type, to add the song to;
/// `opened` is the song when the button was pressed this frame. A click in
/// it types or scrolls; choosing a playlist closes it.
pub(super) fn playlist_menu(app: &mut App, view: &mut View, id: &str, opened: Option<FindRow>) {
    let popup_id = egui::Id::new(("item-playlists", id));
    let Some((popup, row)) = popup(view, popup_id, opened) else {
        return;
    };
    let popup = popup.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    super::menu(popup, view.skin, view.unit, |ui| {
        match playable(app, &row) {
            Some(item) => pick_playlist(app, ui, &item, popup_id.with("query")),
            None => ui.close(),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::{Action, WinampSection};

    fn test_app() -> App {
        let root = std::env::temp_dir().join(format!(
            "spotifast-item-menu-{}-{}",
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

    fn song() -> FindRow {
        FindRow {
            uri: "spotify:track:t".into(),
            label: "Radiohead - Reckoner".into(),
            name: "Reckoner".into(),
            duration_ms: Some(290_000),
            kind: ItemKind::Song,
            album_uri: Some("spotify:album:a".into()),
            artist_uri: Some("spotify:artist:r".into()),
            context: None,
            position: None,
        }
    }

    /// Applies what the menu pushed, as the app does after drawing.
    fn apply(app: &mut App) {
        let ctx = egui::Context::default();
        for action in std::mem::take(&mut app.actions) {
            app.apply(action, &ctx);
        }
    }

    #[test]
    fn each_kind_offers_the_spec_actions() {
        use ItemAction::*;
        assert_eq!(
            actions_for(ItemKind::Song, false),
            &[
                Play,
                PlayNext,
                AddToQueue,
                AddToPlaylist,
                ToggleSaved,
                GoToArtist,
                GoToAlbum,
                Radio,
                CopyLink
            ]
        );
        assert!(actions_for(ItemKind::Song, true).contains(&RemoveFromPlaylist));
        assert_eq!(
            actions_for(ItemKind::Artist, false),
            &[Play, ToggleSaved, Radio, CopyLink]
        );
        assert_eq!(
            actions_for(ItemKind::Album, false),
            &[Play, AddToQueue, ToggleSaved, Radio, CopyLink]
        );
        assert_eq!(
            actions_for(ItemKind::Playlist, false),
            actions_for(ItemKind::Album, false)
        );
    }

    #[test]
    fn go_to_album_opens_it_inside_find_not_the_main_window() {
        let mut app = test_app();
        assert!(!app.settings.find_open);
        perform(&mut app, &song(), ItemAction::GoToAlbum, None);
        assert!(
            !app.actions.iter().any(|a| matches!(a, Action::Open(_))),
            "{:?}",
            app.actions
        );
        assert!(matches!(
            app.actions.as_slice(),
            [Action::ToggleWinampSection(WinampSection::Find)]
        ));
        apply(&mut app);
        assert!(app.settings.find_open);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Album { id, .. } if id == "a"
        ));

        // Already open, FIND is left open and the artist goes on top.
        perform(&mut app, &song(), ItemAction::GoToArtist, None);
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Artist { id, albums: false, name } if id == "r" && name == "Radiohead"
        ));
        assert!(app.winamp.find.back());
        assert!(matches!(app.winamp.find.page(), FindPage::Album { .. }));
        app.backend.shutdown();
    }

    #[test]
    fn radio_opens_a_station_inside_find_for_every_kind() {
        let mut app = test_app();
        app.settings.find_open = true;
        perform(&mut app, &song(), ItemAction::Radio, None);
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Radio { seed, name } if seed == "spotify:track:t" && name == "Reckoner Radio"
        ));
        let artist = FindRow {
            uri: "spotify:artist:r".into(),
            label: "Radiohead".into(),
            kind: ItemKind::Artist,
            ..FindRow::default()
        };
        perform(&mut app, &artist, ItemAction::Radio, None);
        assert!(matches!(
            app.winamp.find.page(),
            FindPage::Radio { seed, .. } if seed == "spotify:artist:r"
        ));
        app.backend.shutdown();
    }

    #[test]
    fn saved_items_offer_to_undo_it_in_their_own_words() {
        let mut app = test_app();
        let artist = FindRow {
            uri: "spotify:artist:r".into(),
            kind: ItemKind::Artist,
            ..FindRow::default()
        };
        let album = FindRow {
            uri: "spotify:album:a".into(),
            kind: ItemKind::Album,
            ..FindRow::default()
        };
        let list = FindRow {
            uri: "spotify:playlist:p".into(),
            kind: ItemKind::Playlist,
            ..FindRow::default()
        };
        let toggle = |app: &App, row: &FindRow| label(app, row, ItemAction::ToggleSaved);
        assert_eq!(toggle(&app, &song()), "Like");
        assert_eq!(toggle(&app, &artist), "Follow");
        assert_eq!(toggle(&app, &album), "Add to Your Library");
        for row in [&song(), &artist, &album, &list] {
            app.saved.insert(row.uri.clone(), true);
        }
        assert_eq!(toggle(&app, &song()), "Unlike");
        assert_eq!(toggle(&app, &artist), "Unfollow");
        assert_eq!(toggle(&app, &album), "Remove from Your Library");
        assert_eq!(toggle(&app, &list), "Remove from Your Library");

        // Unliking shows at once, before Spotify answers.
        perform(&mut app, &song(), ItemAction::ToggleSaved, None);
        assert!(matches!(
            app.actions.as_slice(),
            [Action::ToggleSaved(uri)] if uri == "spotify:track:t"
        ));
        apply(&mut app);
        assert_eq!(toggle(&app, &song()), "Like");
        app.backend.shutdown();
    }

    #[test]
    fn queue_link_and_removal_push_their_actions() {
        let mut app = test_app();
        perform(&mut app, &song(), ItemAction::AddToQueue, None);
        perform(&mut app, &song(), ItemAction::CopyLink, None);
        let own = crate::model::RowContext::Context {
            uri: "spotify:playlist:mine".into(),
            editable_playlist: Some(("mine".into(), Some("snap".into()))),
        };
        assert!(in_own_playlist(Some(&own)));
        assert!(!in_own_playlist(None));
        // The third row of a playlist holding the song more than once.
        let copy = FindRow {
            position: Some(7),
            ..song()
        };
        perform(&mut app, &copy, ItemAction::RemoveFromPlaylist, Some(&own));
        assert!(
            matches!(
                app.actions.as_slice(),
                [
                    Action::AddToQueue { uri, .. },
                    Action::CopyLink(link),
                    Action::RemoveFromPlaylist { playlist_id, uris, positions },
                ] if uri == "spotify:track:t"
                    && link == "spotify:track:t"
                    && playlist_id == "mine"
                    && *uris == ["spotify:track:t"]
                    && *positions == [7]
            ),
            "{:?}",
            app.actions
        );
        app.backend.shutdown();
    }

    #[test]
    fn play_next_and_add_to_playlist_carry_the_whole_song() {
        let mut app = test_app();
        crate::demo::populate(&mut app);
        let track = app.library.liked.items[0].track.clone();
        let row = crate::ui::winamp::find::state::track_row(&track);
        let Some(crate::api::models::PlayableItem::Track(found)) = playable(&app, &row) else {
            panic!("a liked song is held");
        };
        assert_eq!(found, track, "the song as the library holds it");

        // A song the app holds nowhere is sketched from its row.
        let Some(crate::api::models::PlayableItem::Track(sketch)) = playable(&app, &song()) else {
            panic!("a song row always has a song");
        };
        assert_eq!(sketch.uri, "spotify:track:t");
        assert_eq!(sketch.name, "Reckoner");
        assert_eq!(sketch.artists[0].name, "Radiohead");
        assert_eq!(sketch.album.as_ref().unwrap().id, "a");
        let artist = FindRow {
            kind: ItemKind::Artist,
            ..song()
        };
        assert!(playable(&app, &artist).is_none());
        app.backend.shutdown();
    }

    #[test]
    fn play_starts_a_song_in_its_list_and_anything_else_as_itself() {
        let mut app = test_app();
        let album = crate::model::RowContext::Context {
            uri: "spotify:album:a".into(),
            editable_playlist: None,
        };
        perform(&mut app, &song(), ItemAction::Play, Some(&album));
        perform(&mut app, &song(), ItemAction::Play, None);
        let artist = FindRow {
            uri: "spotify:artist:r".into(),
            kind: ItemKind::Artist,
            ..FindRow::default()
        };
        perform(&mut app, &artist, ItemAction::Play, None);
        assert!(
            matches!(
                app.actions.as_slice(),
                [
                    Action::PlayContext { uri: a, offset_uri: Some(o), .. },
                    Action::PlayUris { uris, index: 0 },
                    Action::PlayContext { uri: r, offset_uri: None, .. },
                ] if a == "spotify:album:a" && o == "spotify:track:t"
                    && *uris == ["spotify:track:t"] && r == "spotify:artist:r"
            ),
            "{:?}",
            app.actions
        );
        app.backend.shutdown();
    }

    /// PL plays as its double-click does: the queue from the queued row,
    /// and the playing row starts over where it is, never as a lone song.
    #[test]
    fn play_on_a_pl_row_plays_the_queue_from_it() {
        let mut app = test_app();
        crate::demo::populate(&mut app);
        let queue = Some(&RowContext::Queue);
        let queued = FindRow {
            position: Some(1),
            ..song()
        };
        perform(&mut app, &queued, ItemAction::Play, queue);
        assert!(
            matches!(
                app.actions.as_slice(),
                [Action::PlayFromRow { context: RowContext::Queue, uri, index: 1 }]
                    if uri == "spotify:track:t"
            ),
            "{:?}",
            app.actions
        );
        app.actions.clear();
        let playing = FindRow {
            uri: app.now_playing().unwrap().uri,
            ..song()
        };
        perform(&mut app, &playing, ItemAction::Play, queue);
        assert!(
            matches!(app.actions.as_slice(), [Action::Seek(0)]),
            "{:?}",
            app.actions
        );
        app.actions.clear();
        // A row no longer playing does nothing rather than play alone.
        perform(&mut app, &song(), ItemAction::Play, queue);
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        app.backend.shutdown();
    }

    /// Unfollowing one's own playlist deletes it: not offered here, where
    /// there is no asking first. A playlist someone else made still is.
    #[test]
    fn an_owned_playlist_is_never_unfollowed_from_the_menu() {
        let mut app = test_app();
        crate::demo::populate(&mut app);
        let lists = app.library.playlists.get().unwrap();
        let row = |owner: &str| {
            let list = lists
                .iter()
                .find(|list| list.owner.id.as_deref() == Some(owner))
                .unwrap();
            FindRow {
                uri: list.uri.clone(),
                label: list.name.clone(),
                kind: ItemKind::Playlist,
                ..FindRow::default()
            }
        };
        let (mine, theirs) = (row("demo"), row("spotify"));
        assert!(!available(&app, &mine, ItemAction::ToggleSaved));
        assert!(available(&app, &theirs, ItemAction::ToggleSaved));
        perform(&mut app, &mine, ItemAction::ToggleSaved, None);
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        perform(&mut app, &theirs, ItemAction::ToggleSaved, None);
        assert!(matches!(app.actions.as_slice(), [Action::ToggleSaved(_)]));
        app.backend.shutdown();
    }

    /// A playlist is queued whole, however much of it FIND has loaded:
    /// the app loads the rest (see `App::queue_playlist`).
    #[test]
    fn a_playlist_is_queued_whatever_of_it_is_loaded() {
        let mut app = test_app();
        let list = FindRow {
            uri: "spotify:playlist:p".into(),
            label: "Mix".into(),
            kind: ItemKind::Playlist,
            ..FindRow::default()
        };
        assert!(available(&app, &list, ItemAction::AddToQueue));
        perform(&mut app, &list, ItemAction::AddToQueue, None);
        assert!(
            matches!(
                app.actions.as_slice(),
                [Action::AddToQueue { uri, label }] if uri == "spotify:playlist:p" && label == "Mix"
            ),
            "{:?}",
            app.actions
        );
        app.backend.shutdown();
    }

    /// The labels of the buttons in one frame of `draw`.
    fn buttons(ctx: &egui::Context, draw: impl FnMut(&mut Ui)) -> Vec<String> {
        let mut output = ctx.run_ui(egui::RawInput::default(), draw);
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.unwrap();
        tree.nodes
            .iter()
            .filter_map(|(_, node)| node.label().map(str::to_string))
            .collect()
    }

    /// "New playlist" opens a dialog only the big window draws, so the
    /// mini player's picker leaves it out; the big window's keeps it.
    #[test]
    fn the_mini_players_picker_offers_no_new_playlist() {
        let mut app = test_app();
        crate::demo::populate(&mut app);
        let item = playable(&app, &song()).unwrap();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let id = egui::Id::new("picker");
        let mini = buttons(&ctx, |ui| pick_playlist(&mut app, ui, &item, id));
        assert!(
            !mini.iter().any(|label| label == "New playlist"),
            "{mini:?}"
        );
        assert!(
            mini.iter().any(|label| label == "Late night focus"),
            "{mini:?}"
        );
        let big = buttons(&ctx, |ui| {
            crate::ui::widgets::kept_playlist_picker(
                ui,
                &mut app,
                std::slice::from_ref(&item),
                id,
                true,
            )
        });
        assert!(big.iter().any(|label| label == "New playlist"), "{big:?}");
        app.backend.shutdown();
    }
}
