//! What FIND holds between frames: the box, the mode, and where the user
//! has browsed to. Pure state, no drawing: the view in `mod.rs` reads it
//! and the pages to come push onto its stack.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::api::models::{SearchResults, Track};
use crate::model::{Action, RowContext};

/// The most rows a search shows of each kind.
pub const RESULT_LIMIT: usize = 15;

/// How long the box waits after a key before it searches, as the big
/// window's search field does.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// What the list shows. A kind lists your library's while the box is
/// empty (Liked Songs, followed artists, saved albums, your playlists)
/// and the search's while it is not; RECENT, only with an empty box, the
/// songs played lately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FindMode {
    Recent,
    #[default]
    Songs,
    Artists,
    Albums,
    Lists,
}

const EMPTY_MODES: [FindMode; 5] = [
    FindMode::Recent,
    FindMode::Songs,
    FindMode::Artists,
    FindMode::Albums,
    FindMode::Lists,
];
const SEARCH_MODES: [FindMode; 4] = [
    FindMode::Songs,
    FindMode::Artists,
    FindMode::Albums,
    FindMode::Lists,
];
/// An artist's page: their top songs, or their albums.
const ARTIST_MODES: [FindMode; 2] = [FindMode::Songs, FindMode::Albums];

impl FindMode {
    /// The mode row's word for it, in the skin's capitals.
    pub fn label(self) -> &'static str {
        match self {
            FindMode::Recent => "RECENT",
            FindMode::Songs => "SONGS",
            FindMode::Artists => "ARTISTS",
            FindMode::Albums => "ALBUMS",
            FindMode::Lists => "LISTS",
        }
    }

    /// Whether it is a kind of thing, listed from the library or a search.
    pub fn is_kind(self) -> bool {
        SEARCH_MODES.contains(&self)
    }
}

/// A song's row: its artists and name, its length, and where it is from.
pub fn track_row(track: &Track) -> FindRow {
    let present = |uri: Option<&String>| uri.filter(|uri| !uri.is_empty()).cloned();
    FindRow {
        uri: track.uri.clone(),
        label: by(&track.artist_names(), &track.name),
        name: track.name.clone(),
        duration_ms: Some(track.duration_ms),
        kind: ItemKind::Song,
        album_uri: present(track.album.as_ref().map(|album| &album.uri)),
        artist_uri: present(track.artists.first().and_then(|artist| artist.uri.as_ref())),
        context: None,
        position: None,
    }
}

/// "Who - what", or just what when nobody is named.
pub fn by(who: &str, what: &str) -> String {
    if who.is_empty() {
        what.to_string()
    } else {
        format!("{who} - {what}")
    }
}

/// What a row is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemKind {
    #[default]
    Song,
    Artist,
    Album,
    Playlist,
}

/// One line of FIND's list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindRow {
    pub uri: String,
    pub label: String,
    /// The item's own name, for the title line once it is opened; the
    /// label may lead with its artists. Empty when the label is the name.
    pub name: String,
    pub duration_ms: Option<u32>,
    pub kind: ItemKind,
    /// A song's album and first artist, for going to them.
    pub album_uri: Option<String>,
    pub artist_uri: Option<String>,
    /// The context a Liked Songs row plays in: the collection, once the
    /// list is whole, as the big window's Liked Songs page plays it.
    pub context: Option<String>,
    /// Where the row sits in the list it was drawn from, as that list
    /// counts: a playlist's server position, so taking it out takes only
    /// this copy; PL's place in the queue, so playing it plays from there.
    pub position: Option<u32>,
}

/// What choosing a row does: play, or open a page inside FIND.
#[derive(Clone, Debug)]
pub enum Choice {
    Play(Action),
    Open(FindPage),
}

/// Where FIND has browsed to; the bottom of the stack is the search or
/// library itself.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FindPage {
    Top,
    Artist {
        id: String,
        name: String,
        /// Listing the artist's albums rather than top songs.
        albums: bool,
    },
    Album {
        id: String,
        name: String,
    },
    Playlist {
        id: String,
        name: String,
    },
    Radio {
        seed: String,
        name: String,
    },
}

impl FindPage {
    /// The big window's page that loads what this one lists; `None` for
    /// the top, which lists the search or the library.
    pub fn as_page(&self) -> Option<crate::model::Page> {
        use crate::model::Page;
        Some(match self {
            FindPage::Top => return None,
            FindPage::Artist { id, .. } => Page::Artist(id.clone()),
            FindPage::Album { id, .. } => Page::Album(id.clone()),
            FindPage::Playlist { id, .. } => Page::Playlist(id.clone()),
            FindPage::Radio { seed, .. } => Page::Radio(seed.clone()),
        })
    }
}

pub struct FindState {
    pub query: String,
    pub mode: FindMode,
    /// Never empty; `stack[0]` is `Top`.
    pub stack: Vec<FindPage>,
    /// The row each page under the top one was left from, by URI, for
    /// BACK to return to; one fewer than `stack`.
    pub trail: Vec<Option<String>>,
    /// The row BACK has just returned to, for the view to find and pick.
    pub reselect: Option<String>,
    pub selected: Option<usize>,
    /// The library kind last listed, which an empty box returns to.
    pub library_kind: FindMode,
    /// The rows last built for the list and what they were built from,
    /// so a long list is not rebuilt every frame.
    pub rows: RefCell<Option<(u64, Arc<Vec<FindRow>>)>>,
    /// Whether keys go to the box.
    pub focused: bool,
    /// Focus the box on the next frame it is open.
    pub focus_requested: bool,
    /// When the box last changed with its search still to run.
    pub pending: Option<Instant>,
    /// The last answer shown, kept on screen while the answer for what
    /// the box now holds is on its way.
    pub held: Held,
}

/// The last answer that came for what the box held, kind by kind: a
/// search arrives in two halves, the catalogue (songs, artists, albums)
/// and the playlists, either first, and each half replaces only its own.
#[derive(Debug, Default)]
pub struct Held {
    pub results: SearchResults,
    /// The search each half came from.
    pub catalogue: Option<u64>,
    pub lists: Option<u64>,
}

impl Held {
    /// Takes the half of `results` that search `serial` has delivered.
    /// Returns whether the rows of `mode` changed.
    pub fn take(
        &mut self,
        serial: u64,
        results: &SearchResults,
        catalogue: bool,
        lists: bool,
        mode: FindMode,
    ) -> bool {
        let mut changed = false;
        if catalogue && self.catalogue != Some(serial) {
            self.results.tracks.clone_from(&results.tracks);
            self.results.artists.clone_from(&results.artists);
            self.results.albums.clone_from(&results.albums);
            self.catalogue = Some(serial);
            changed |= mode != FindMode::Lists;
        }
        if lists && self.lists != Some(serial) {
            self.results.playlists.clone_from(&results.playlists);
            self.lists = Some(serial);
            changed |= mode == FindMode::Lists;
        }
        changed
    }
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: FindMode::default(),
            stack: vec![FindPage::Top],
            trail: Vec::new(),
            reselect: None,
            selected: None,
            library_kind: FindMode::default(),
            rows: RefCell::new(None),
            focused: false,
            focus_requested: false,
            pending: None,
            held: Held::default(),
        }
    }
}

impl FindState {
    /// Puts `query` in the box and searches for it at once.
    pub fn set_query(&mut self, query: String) -> Vec<Action> {
        self.edit(query);
        self.pending = None;
        self.search()
    }

    /// Puts `query` in the box as a key does: the search waits for a
    /// pause, which `due` reports.
    pub fn type_query(&mut self, query: String, now: Instant) {
        self.edit(query);
        self.pending = (!self.query.trim().is_empty()).then_some(now);
    }

    /// Enter: the search for what the box holds, without waiting.
    pub fn submit(&mut self) -> Vec<Action> {
        self.pending = None;
        self.search()
    }

    /// The search a pause in typing has made due, once.
    pub fn due(&mut self, now: Instant) -> Vec<Action> {
        match self.pending {
            Some(typed) if now.saturating_duration_since(typed) >= SEARCH_DEBOUNCE => self.submit(),
            _ => Vec::new(),
        }
    }

    /// How long until the waiting search is due, to wake for it.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending
            .map(|typed| SEARCH_DEBOUNCE.saturating_sub(now.saturating_duration_since(typed)))
    }

    /// The box changed: the list goes back to the top and keeps its kind,
    /// RECENT turning into SONGS once there is something to search for,
    /// and an emptied box lets go of the answer it showed.
    fn edit(&mut self, query: String) {
        if query.trim().is_empty() {
            self.held = Held::default();
        } else if !self.mode.is_kind() {
            self.mode = FindMode::Songs;
        }
        if query != self.query {
            self.selected = None;
        }
        self.query = query;
        self.stack.truncate(1);
        self.trail.clear();
        self.reselect = None;
        self.note_kind();
    }

    /// Remembers the library kind on show.
    fn note_kind(&mut self) {
        if self.query.trim().is_empty() && self.mode.is_kind() {
            self.library_kind = self.mode;
        }
    }

    /// The library, in the kind last listed: back to the top with the box
    /// emptied.
    pub fn show_library(&mut self) {
        // Taken first: emptying the box would note a search's kind.
        let kind = self.library_kind;
        self.mode = kind;
        self.edit(String::new());
        self.pending = None;
        self.selected = None;
    }

    /// The songs played lately: back to the top with the box emptied.
    pub fn show_recent(&mut self) {
        self.show_library();
        self.set_mode(FindMode::Recent);
    }

    fn search(&self) -> Vec<Action> {
        let query = self.query.trim();
        if query.is_empty() {
            Vec::new()
        } else {
            vec![Action::FindSearch(query.to_string())]
        }
    }

    /// The modes the row offers. Inside a page only its own: an artist's
    /// songs and albums, and none for the others. At the top they follow
    /// what the box holds.
    pub fn modes(&self) -> &'static [FindMode] {
        match self.page() {
            FindPage::Artist { .. } => &ARTIST_MODES,
            FindPage::Top if self.query.trim().is_empty() => &EMPTY_MODES,
            FindPage::Top => &SEARCH_MODES,
            _ => &[],
        }
    }

    /// The mode the row picks out.
    pub fn shown_mode(&self) -> FindMode {
        match self.page() {
            FindPage::Artist { albums: true, .. } => FindMode::Albums,
            FindPage::Artist { .. } => FindMode::Songs,
            _ => self.mode,
        }
    }

    /// The next mode along the row, or the one before, wrapping.
    pub fn cycle_mode(&mut self, forward: bool) {
        let modes = self.modes();
        if modes.is_empty() {
            return;
        }
        let shown = self.shown_mode();
        let at = modes.iter().position(|mode| *mode == shown).unwrap_or(0);
        let next = if forward {
            (at + 1) % modes.len()
        } else {
            (at + modes.len() - 1) % modes.len()
        };
        self.pick_mode(modes[next]);
    }

    /// A mode picked on the row: at the top it switches the list; on an
    /// artist's page between their songs and albums. It never leaves a
    /// page.
    pub fn pick_mode(&mut self, mode: FindMode) {
        match self.page() {
            FindPage::Top => self.set_mode(mode),
            FindPage::Artist { .. } => self.set_artist_albums(mode == FindMode::Albums),
            _ => {}
        }
    }

    /// Switches the top list to another mode.
    pub fn set_mode(&mut self, mode: FindMode) {
        if mode != self.mode {
            self.mode = mode;
            self.selected = None;
        }
        self.note_kind();
    }

    /// Back out of a page. False at the top, where there is nowhere to go.
    /// The row the page was opened from waits in `reselect` for the view,
    /// which knows where it now is.
    pub fn back(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.selected = None;
            self.reselect = self.trail.pop().flatten();
            true
        } else {
            false
        }
    }

    /// Opens a page on top of the one shown, from its first row.
    pub fn open(&mut self, page: FindPage) {
        self.open_from(page, None);
    }

    /// Opens a page from the row with `uri`, which BACK returns to.
    pub fn open_from(&mut self, page: FindPage, uri: Option<String>) {
        self.trail.push(uri);
        self.selected = None;
        self.reselect = None;
        self.stack.push(page);
    }

    /// The page shown.
    pub fn page(&self) -> &FindPage {
        self.stack.last().unwrap_or(&FindPage::Top)
    }

    /// An artist's page between their top songs and their albums.
    pub fn set_artist_albums(&mut self, to: bool) {
        if let Some(FindPage::Artist { albums, .. }) = self.stack.last_mut()
            && *albums != to
        {
            *albums = to;
            self.selected = None;
        }
    }

    /// What double-click, Enter or PLAY on `row`, the `index`th of `rows`,
    /// does. A song plays in the album or playlist it is listed in; an
    /// artist's top songs and a radio, which Spotify has no context for,
    /// play as a list from the chosen song; a song found or recently
    /// played plays on its own, and Spotify's autoplay carries on after
    /// it; Liked Songs (SONGS with the box empty) play as the big window's
    /// page plays them. Artists,
    /// albums and playlists open inside FIND.
    pub fn choose(&self, row: &FindRow, rows: &[FindRow], index: usize) -> Choice {
        let id = || {
            crate::util::uri_id(&row.uri)
                .unwrap_or_default()
                .to_string()
        };
        let name = || {
            if row.name.is_empty() {
                row.label.clone()
            } else {
                row.name.clone()
            }
        };
        match row.kind {
            ItemKind::Artist => {
                return Choice::Open(FindPage::Artist {
                    id: id(),
                    name: name(),
                    albums: false,
                });
            }
            ItemKind::Album => {
                return Choice::Open(FindPage::Album {
                    id: id(),
                    name: name(),
                });
            }
            ItemKind::Playlist => {
                return Choice::Open(FindPage::Playlist {
                    id: id(),
                    name: name(),
                });
            }
            ItemKind::Song => {}
        }
        // The songs of the list, and the chosen one's place among them.
        let songs = || {
            let uris: Vec<String> = rows
                .iter()
                .filter(|row| row.kind == ItemKind::Song)
                .map(|row| row.uri.clone())
                .collect();
            let at = rows[..index.min(rows.len())]
                .iter()
                .filter(|row| row.kind == ItemKind::Song)
                .count();
            (uris, at as u32)
        };
        let in_context = |uri: String| {
            Choice::Play(Action::PlayContext {
                uri,
                offset_uri: Some(row.uri.clone()),
                offset_index: None,
            })
        };
        match self.page() {
            FindPage::Album { id, .. } => in_context(format!("spotify:album:{id}")),
            FindPage::Playlist { id, .. } => in_context(format!("spotify:playlist:{id}")),
            FindPage::Artist { .. } | FindPage::Radio { .. } => {
                let (uris, index) = songs();
                Choice::Play(Action::PlayUris { uris, index })
            }
            FindPage::Top if self.mode == FindMode::Songs && self.query.trim().is_empty() => {
                let (uris, index) = songs();
                let context = match &row.context {
                    Some(uri) => RowContext::Context {
                        uri: uri.clone(),
                        editable_playlist: None,
                    },
                    None => RowContext::Uris(Arc::from(uris)),
                };
                Choice::Play(Action::PlayFromRow {
                    context,
                    uri: row.uri.clone(),
                    index,
                })
            }
            FindPage::Top => Choice::Play(Action::PlayUris {
                uris: vec![row.uri.clone()],
                index: 0,
            }),
        }
    }

    /// The search results of the mode's kind, the first `RESULT_LIMIT`.
    pub fn top_search_rows(&self, results: &SearchResults) -> Vec<FindRow> {
        fn first<T>(page: &Option<crate::api::models::Page<T>>) -> &[T] {
            let items = page.as_ref().map_or(&[][..], |page| &page.items[..]);
            &items[..items.len().min(RESULT_LIMIT)]
        }
        fn names(artists: &[crate::api::models::ArtistRef]) -> String {
            crate::api::models::join_names(artists.iter().map(|artist| artist.name.as_str()))
        }
        let row = |uri: &str, label: String, kind: ItemKind| FindRow {
            uri: uri.to_string(),
            label,
            kind,
            ..FindRow::default()
        };
        match self.mode {
            FindMode::Recent => Vec::new(),
            FindMode::Songs => first(&results.tracks).iter().map(track_row).collect(),
            FindMode::Artists => first(&results.artists)
                .iter()
                .map(|artist| row(&artist.uri, artist.name.clone(), ItemKind::Artist))
                .collect(),
            FindMode::Albums => first(&results.albums)
                .iter()
                .map(|album| FindRow {
                    name: album.name.clone(),
                    ..row(
                        &album.uri,
                        by(&names(&album.artists), &album.name),
                        ItemKind::Album,
                    )
                })
                .collect(),
            FindMode::Lists => first(&results.playlists)
                .iter()
                .map(|list| row(&list.uri, list.name.clone(), ItemKind::Playlist))
                .collect(),
        }
    }

    /// The section's name, and the page's while inside one. A hyphen, not
    /// a dot: the skin's font has no middle dot.
    pub fn title(&self) -> String {
        match self.stack.last() {
            Some(
                FindPage::Artist { name, .. }
                | FindPage::Album { name, .. }
                | FindPage::Playlist { name, .. }
                | FindPage::Radio { name, .. },
            ) => format!("FIND - {name}"),
            _ => "FIND".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Search results with `tracks` songs and `artists` artists, as Spotify
    /// sends them.
    fn search_results(tracks: usize, artists: usize) -> SearchResults {
        let tracks: Vec<String> = (0..tracks)
            .map(|i| {
                format!(
                    r#"{{"id":"t{i}","name":"Song {i}","uri":"spotify:track:t{i}","duration_ms":{},"artists":[{{"id":"a{i}","name":"Artist {i}","uri":"spotify:artist:a{i}"}}],"album":{{"id":"al{i}","name":"Album {i}","uri":"spotify:album:al{i}"}}}}"#,
                    180_000 + i
                )
            })
            .collect();
        let artists: Vec<String> = (0..artists)
            .map(|i| format!(r#"{{"id":"a{i}","name":"Artist {i}","uri":"spotify:artist:a{i}"}}"#))
            .collect();
        let json = format!(
            r#"{{"tracks":{{"items":[{}],"total":{},"limit":20,"offset":0}},"artists":{{"items":[{}],"total":{},"limit":20,"offset":0}}}}"#,
            tracks.join(","),
            tracks.len(),
            artists.join(","),
            artists.len()
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn typing_keeps_the_kind_and_recent_becomes_songs() {
        let mut s = FindState::default();
        assert_eq!(s.mode, FindMode::Songs, "an empty box opens on Liked Songs");
        assert_eq!(
            s.modes(),
            &[
                FindMode::Recent,
                FindMode::Songs,
                FindMode::Artists,
                FindMode::Albums,
                FindMode::Lists
            ]
        );
        s.set_mode(FindMode::Artists);
        assert!(
            matches!(s.set_query("radiohead".into()).as_slice(), [Action::FindSearch(q)] if q == "radiohead")
        );
        assert_eq!(s.mode, FindMode::Artists, "typing searches the kind picked");
        assert_eq!(s.modes(), &SEARCH_MODES);
        s.set_mode(FindMode::Albums);
        assert!(s.set_query(String::new()).is_empty());
        assert_eq!(s.mode, FindMode::Albums, "emptying the box keeps the kind");
        s.set_mode(FindMode::Recent);
        s.set_query(String::new());
        assert_eq!(s.mode, FindMode::Recent, "an empty box keeps RECENT");
        s.set_query("x".into());
        assert_eq!(
            s.mode,
            FindMode::Songs,
            "from RECENT, typing searches songs"
        );
    }

    #[test]
    fn show_library_returns_to_the_kind_last_listed() {
        let mut s = FindState::default();
        s.show_library();
        assert_eq!(s.mode, FindMode::Songs, "Liked Songs at first");
        s.set_mode(FindMode::Lists);
        s.set_mode(FindMode::Recent);
        s.set_query("radio".into());
        s.set_mode(FindMode::Artists);
        s.open(FindPage::Album {
            id: "a".into(),
            name: "A".into(),
        });
        s.selected = Some(2);
        s.show_library();
        assert_eq!(
            s.mode,
            FindMode::Lists,
            "a search's kind is not the library's"
        );
        assert!(s.query.is_empty());
        assert_eq!(s.stack, [FindPage::Top]);
        assert_eq!(s.selected, None);
        assert!(s.due(Instant::now() + Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn a_page_shows_only_its_own_modes_and_keeps_its_stack() {
        let mut s = FindState::default();
        s.open(FindPage::Artist {
            id: "r".into(),
            name: "R".into(),
            albums: false,
        });
        assert_eq!(s.modes(), &[FindMode::Songs, FindMode::Albums]);
        s.cycle_mode(true);
        assert!(matches!(s.page(), FindPage::Artist { albums: true, .. }));
        s.open(FindPage::Album {
            id: "a".into(),
            name: "A".into(),
        });
        assert!(s.modes().is_empty());
        s.cycle_mode(true);
        s.pick_mode(FindMode::Lists);
        assert_eq!(s.stack.len(), 3, "Tab and the row never leave a page");
        assert_eq!(s.mode, FindMode::Songs);
    }

    #[test]
    fn search_rows_are_the_modes_kind_and_at_most_fifteen() {
        let results = search_results(16, 3);
        let mut s = FindState::default();
        s.set_query("x".into());
        let rows = s.top_search_rows(&results);
        assert_eq!(rows.len(), RESULT_LIMIT);
        assert!(rows.iter().all(|r| r.kind == ItemKind::Song));
        assert_eq!(rows[0].uri, "spotify:track:t0");
        assert_eq!(rows[0].duration_ms, Some(180_000));
        assert_eq!(rows[0].album_uri.as_deref(), Some("spotify:album:al0"));
        assert_eq!(rows[0].artist_uri.as_deref(), Some("spotify:artist:a0"));
        s.mode = FindMode::Artists;
        let rows = s.top_search_rows(&results);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.kind == ItemKind::Artist));
        s.mode = FindMode::Albums;
        assert!(s.top_search_rows(&results).is_empty(), "no albums came");
    }

    #[test]
    fn a_keystroke_waits_for_a_pause_before_searching() {
        let mut s = FindState::default();
        let start = Instant::now();
        s.type_query("rad".into(), start);
        assert_eq!(s.mode, FindMode::Songs, "the mode row follows at once");
        assert!(s.due(start + Duration::from_millis(100)).is_empty());
        s.type_query("radio".into(), start + Duration::from_millis(200));
        assert!(
            s.due(start + Duration::from_millis(400)).is_empty(),
            "the second key restarted the wait"
        );
        assert!(matches!(
            s.due(start + Duration::from_millis(500)).as_slice(),
            [Action::FindSearch(q)] if q == "radio"
        ));
        assert!(
            s.due(start + Duration::from_secs(5)).is_empty(),
            "one pause, one search"
        );
    }

    #[test]
    fn enter_searches_at_once_and_an_empty_box_never_searches() {
        let mut s = FindState::default();
        let start = Instant::now();
        s.type_query("radio".into(), start);
        assert!(matches!(s.submit().as_slice(), [Action::FindSearch(q)] if q == "radio"));
        assert!(s.due(start + Duration::from_secs(1)).is_empty());
        s.type_query("   ".into(), start);
        assert!(s.submit().is_empty());
        assert!(s.due(start + Duration::from_secs(1)).is_empty());
    }

    #[test]
    fn modes_cycle_within_the_row_shown() {
        let mut s = FindState::default();
        s.cycle_mode(true);
        assert_eq!(s.mode, FindMode::Artists);
        s.set_mode(FindMode::Lists);
        s.cycle_mode(true);
        assert_eq!(s.mode, FindMode::Recent);
        s.set_query("x".into());
        assert_eq!(s.mode, FindMode::Songs);
        s.cycle_mode(false);
        assert_eq!(s.mode, FindMode::Lists);
        s.cycle_mode(true);
        assert_eq!(s.mode, FindMode::Songs);
    }

    #[test]
    fn the_title_names_the_page_and_a_new_query_returns_to_the_top() {
        let mut s = FindState::default();
        assert_eq!(s.title(), "FIND");
        s.stack.push(FindPage::Album {
            id: "a".into(),
            name: "In Rainbows".into(),
        });
        assert_eq!(s.title(), "FIND - In Rainbows");
        assert!(s.back());
        assert_eq!(s.title(), "FIND");
        assert!(!s.back(), "the top stays");
        assert_eq!(s.stack, [FindPage::Top]);
        s.stack.push(FindPage::Radio {
            seed: "spotify:track:t".into(),
            name: "Song".into(),
        });
        s.set_query("x".into());
        assert_eq!(s.stack, [FindPage::Top]);
    }

    fn row() -> FindRow {
        FindRow::default()
    }

    fn song(id: &str) -> FindRow {
        FindRow {
            uri: format!("spotify:track:{id}"),
            label: id.to_uppercase(),
            kind: ItemKind::Song,
            ..row()
        }
    }

    #[test]
    fn choosing_an_album_opens_it_and_back_returns() {
        let mut s = FindState::default();
        let album = FindRow {
            uri: "spotify:album:a".into(),
            label: "In Rainbows".into(),
            kind: ItemKind::Album,
            ..row()
        };
        assert!(
            matches!(s.choose(&album, std::slice::from_ref(&album), 0), Choice::Open(FindPage::Album { id, .. }) if id == "a")
        );
        s.open(FindPage::Album {
            id: "a".into(),
            name: "In Rainbows".into(),
        });
        assert_eq!(s.title(), "FIND - In Rainbows");
        assert!(s.back());
        assert!(!s.back());
        assert_eq!(s.page(), &FindPage::Top);
    }

    #[test]
    fn a_rows_own_name_titles_the_page_it_opens() {
        let s = FindState::default();
        let album = FindRow {
            uri: "spotify:album:a".into(),
            label: "Radiohead - In Rainbows".into(),
            name: "In Rainbows".into(),
            kind: ItemKind::Album,
            ..row()
        };
        assert!(matches!(
            s.choose(&album, std::slice::from_ref(&album), 0),
            Choice::Open(FindPage::Album { name, .. }) if name == "In Rainbows"
        ));
        let artist = FindRow {
            uri: "spotify:artist:r".into(),
            label: "Radiohead".into(),
            kind: ItemKind::Artist,
            ..row()
        };
        assert!(matches!(
            s.choose(&artist, std::slice::from_ref(&artist), 0),
            Choice::Open(FindPage::Artist { id, name, albums: false }) if id == "r" && name == "Radiohead"
        ));
        let list = FindRow {
            uri: "spotify:playlist:p".into(),
            label: "Mix".into(),
            kind: ItemKind::Playlist,
            ..row()
        };
        assert!(matches!(
            s.choose(&list, std::slice::from_ref(&list), 0),
            Choice::Open(FindPage::Playlist { id, .. }) if id == "p"
        ));
    }

    #[test]
    fn a_song_inside_an_album_plays_in_the_album() {
        let mut s = FindState::default();
        s.open(FindPage::Album {
            id: "a".into(),
            name: "A".into(),
        });
        let song = FindRow {
            uri: "spotify:track:t".into(),
            kind: ItemKind::Song,
            ..row()
        };
        assert!(matches!(s.choose(&song, std::slice::from_ref(&song), 0),
            Choice::Play(Action::PlayContext { uri, offset_uri: Some(o), .. }) if uri == "spotify:album:a" && o == "spotify:track:t"));
        s.open(FindPage::Playlist {
            id: "p".into(),
            name: "P".into(),
        });
        assert!(matches!(s.choose(&song, std::slice::from_ref(&song), 0),
            Choice::Play(Action::PlayContext { uri, offset_uri: Some(o), offset_index: None }) if uri == "spotify:playlist:p" && o == "spotify:track:t"));
    }

    #[test]
    fn a_searched_song_plays_on_its_own() {
        let mut s = FindState::default();
        s.set_query("x".into());
        let rows = [song("a"), song("b")];
        assert!(matches!(
            s.choose(&rows[1], &rows, 1),
            Choice::Play(Action::PlayUris { uris, index: 0 }) if uris == ["spotify:track:b"]
        ));
        let mut s = FindState::default();
        s.set_mode(FindMode::Recent);
        assert!(matches!(
            s.choose(&rows[1], &rows, 1),
            Choice::Play(Action::PlayUris { uris, index: 0 }) if uris == ["spotify:track:b"]
        ));
    }

    #[test]
    fn top_songs_and_radio_play_every_song_from_the_chosen_one() {
        let mut s = FindState::default();
        let rows = [song("a"), song("b"), song("c")];
        for page in [
            FindPage::Artist {
                id: "r".into(),
                name: "R".into(),
                albums: false,
            },
            FindPage::Radio {
                seed: "spotify:track:a".into(),
                name: "A Radio".into(),
            },
        ] {
            s.open(page);
            assert!(matches!(
                s.choose(&rows[2], &rows, 2),
                Choice::Play(Action::PlayUris { uris, index: 2 })
                    if uris == ["spotify:track:a", "spotify:track:b", "spotify:track:c"]
            ));
        }
    }

    #[test]
    fn a_liked_song_plays_in_liked_songs_as_the_big_window_does() {
        let s = FindState::default();
        assert_eq!(s.mode, FindMode::Songs);
        let rows = [song("a"), song("b")];
        // Not yet whole: the songs loaded, as a list.
        assert!(matches!(
            s.choose(&rows[1], &rows, 1),
            Choice::Play(Action::PlayFromRow { context: RowContext::Uris(uris), uri, index: 1 })
                if uris.as_ref() == ["spotify:track:a", "spotify:track:b"] && uri == "spotify:track:b"
        ));
        // Whole: the collection itself.
        let collection = "spotify:user:me:collection".to_string();
        let rows: Vec<FindRow> = rows
            .into_iter()
            .map(|row| FindRow {
                context: Some(collection.clone()),
                ..row
            })
            .collect();
        assert!(matches!(
            s.choose(&rows[0], &rows, 0),
            Choice::Play(Action::PlayFromRow { context: RowContext::Context { uri, editable_playlist: None }, index: 0, .. })
                if uri == collection
        ));
    }

    #[test]
    fn an_artist_page_toggles_between_songs_and_albums() {
        let mut s = FindState::default();
        s.open(FindPage::Artist {
            id: "r".into(),
            name: "R".into(),
            albums: false,
        });
        s.selected = Some(3);
        s.set_artist_albums(true);
        assert!(matches!(s.page(), FindPage::Artist { albums: true, .. }));
        assert_eq!(s.selected, None);
        let album = FindRow {
            uri: "spotify:album:a".into(),
            label: "A".into(),
            kind: ItemKind::Album,
            ..row()
        };
        assert!(matches!(
            s.choose(&album, std::slice::from_ref(&album), 0),
            Choice::Open(FindPage::Album { .. })
        ));
        s.set_artist_albums(false);
        assert!(matches!(s.page(), FindPage::Artist { albums: false, .. }));
    }

    #[test]
    fn back_returns_to_the_row_that_was_opened_by_uri() {
        let mut s = FindState {
            selected: Some(4),
            ..FindState::default()
        };
        s.open_from(
            FindPage::Artist {
                id: "r".into(),
                name: "R".into(),
                albums: false,
            },
            Some("spotify:artist:r".into()),
        );
        assert_eq!(s.selected, None);
        s.open_from(
            FindPage::Album {
                id: "a".into(),
                name: "A".into(),
            },
            Some("spotify:album:a".into()),
        );
        assert!(s.back());
        assert_eq!(s.reselect.as_deref(), Some("spotify:album:a"));
        assert!(s.back());
        assert_eq!(s.reselect.as_deref(), Some("spotify:artist:r"));
        assert_eq!(s.selected, None, "the view finds the row by its URI");
    }
}
