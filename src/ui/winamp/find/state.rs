//! What FIND holds between frames: the box, the mode, and where the user
//! has browsed to. Pure state, no drawing: the view in `mod.rs` reads it
//! and the pages to come push onto its stack.

use std::time::{Duration, Instant};

use crate::api::models::SearchResults;
use crate::model::Action;

/// The most rows a search shows of each kind.
pub const RESULT_LIMIT: usize = 15;

/// How long the box waits after a key before it searches, as the big
/// window's search field does.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// What the list shows: the library or recent songs while the box is
/// empty, one kind of search result while it is not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FindMode {
    #[default]
    Library,
    Recent,
    Songs,
    Artists,
    Albums,
    Lists,
}

const EMPTY_MODES: [FindMode; 2] = [FindMode::Library, FindMode::Recent];
const SEARCH_MODES: [FindMode; 4] = [
    FindMode::Songs,
    FindMode::Artists,
    FindMode::Albums,
    FindMode::Lists,
];

impl FindMode {
    /// The mode row's word for it, in the skin's capitals.
    pub fn label(self) -> &'static str {
        match self {
            FindMode::Library => "LIBRARY",
            FindMode::Recent => "RECENT",
            FindMode::Songs => "SONGS",
            FindMode::Artists => "ARTISTS",
            FindMode::Albums => "ALBUMS",
            FindMode::Lists => "LISTS",
        }
    }

    /// Whether it lists search results.
    pub fn is_search(self) -> bool {
        SEARCH_MODES.contains(&self)
    }
}

/// What a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Song,
    Artist,
    Album,
    Playlist,
}

/// One line of FIND's list.
#[derive(Clone, Debug, PartialEq)]
pub struct FindRow {
    pub uri: String,
    pub label: String,
    pub duration_ms: Option<u32>,
    pub kind: ItemKind,
    /// A song's album and first artist, for going to them.
    pub album_uri: Option<String>,
    pub artist_uri: Option<String>,
}

/// Where FIND has browsed to; the bottom of the stack is the search or
/// library itself.
#[derive(Clone, Debug, PartialEq)]
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

pub struct FindState {
    pub query: String,
    pub mode: FindMode,
    /// Never empty; `stack[0]` is `Top`.
    pub stack: Vec<FindPage>,
    pub selected: Option<usize>,
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
            selected: None,
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

    /// The box changed: the mode row follows it, the list goes back to the
    /// top, and an emptied box lets go of the answer it showed.
    fn edit(&mut self, query: String) {
        if query.trim().is_empty() {
            if !EMPTY_MODES.contains(&self.mode) {
                self.mode = FindMode::Library;
            }
            self.held = Held::default();
        } else if !self.mode.is_search() {
            self.mode = FindMode::Songs;
        }
        if query != self.query {
            self.selected = None;
        }
        self.query = query;
        self.stack.truncate(1);
    }

    fn search(&self) -> Vec<Action> {
        let query = self.query.trim();
        if query.is_empty() {
            Vec::new()
        } else {
            vec![Action::FindSearch(query.to_string())]
        }
    }

    /// The modes the row offers for what the box holds.
    pub fn modes(&self) -> &'static [FindMode] {
        if self.query.trim().is_empty() {
            &EMPTY_MODES
        } else {
            &SEARCH_MODES
        }
    }

    /// The next mode along the row, or the one before, wrapping.
    pub fn cycle_mode(&mut self, forward: bool) {
        let modes = self.modes();
        let at = modes
            .iter()
            .position(|mode| *mode == self.mode)
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % modes.len()
        } else {
            (at + modes.len() - 1) % modes.len()
        };
        self.set_mode(modes[next]);
    }

    /// Switches the list to another mode.
    pub fn set_mode(&mut self, mode: FindMode) {
        if mode != self.mode {
            self.mode = mode;
            self.selected = None;
        }
    }

    /// Back out of a page. False at the top, where there is nowhere to go.
    pub fn back(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.selected = None;
            true
        } else {
            false
        }
    }

    /// The search results of the mode's kind, the first `RESULT_LIMIT`.
    pub fn top_search_rows(&self, results: &SearchResults) -> Vec<FindRow> {
        fn first<T>(page: &Option<crate::api::models::Page<T>>) -> &[T] {
            let items = page.as_ref().map_or(&[][..], |page| &page.items[..]);
            &items[..items.len().min(RESULT_LIMIT)]
        }
        fn names(artists: &[crate::api::models::ArtistRef]) -> String {
            artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
        fn by(who: String, what: &str) -> String {
            if who.is_empty() {
                what.to_string()
            } else {
                format!("{who} - {what}")
            }
        }
        let row = |uri: &str, label: String, kind: ItemKind| FindRow {
            uri: uri.to_string(),
            label,
            duration_ms: None,
            kind,
            album_uri: None,
            artist_uri: None,
        };
        let present = |uri: &Option<String>| uri.clone().filter(|uri| !uri.is_empty());
        match self.mode {
            FindMode::Library | FindMode::Recent => Vec::new(),
            FindMode::Songs => first(&results.tracks)
                .iter()
                .map(|track| FindRow {
                    duration_ms: Some(track.duration_ms),
                    album_uri: present(&track.album.as_ref().map(|album| album.uri.clone())),
                    artist_uri: present(
                        &track.artists.first().and_then(|artist| artist.uri.clone()),
                    ),
                    ..row(
                        &track.uri,
                        by(names(&track.artists), &track.name),
                        ItemKind::Song,
                    )
                })
                .collect(),
            FindMode::Artists => first(&results.artists)
                .iter()
                .map(|artist| row(&artist.uri, artist.name.clone(), ItemKind::Artist))
                .collect(),
            FindMode::Albums => first(&results.albums)
                .iter()
                .map(|album| {
                    row(
                        &album.uri,
                        by(names(&album.artists), &album.name),
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
    fn typing_switches_to_search_modes_and_clearing_returns_to_library() {
        let mut s = FindState::default();
        assert_eq!(s.mode, FindMode::Library);
        assert!(
            matches!(s.set_query("radiohead".into()).as_slice(), [Action::FindSearch(q)] if q == "radiohead")
        );
        assert_eq!(s.mode, FindMode::Songs);
        s.mode = FindMode::Albums;
        s.set_query("radioheads".into());
        assert_eq!(s.mode, FindMode::Albums);
        assert!(s.set_query(String::new()).is_empty());
        assert_eq!(s.mode, FindMode::Library);
        assert_eq!(s.modes(), &[FindMode::Library, FindMode::Recent]);
        s.mode = FindMode::Recent;
        s.set_query(String::new());
        assert_eq!(s.mode, FindMode::Recent, "an empty box keeps RECENT");
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
        assert_eq!(s.mode, FindMode::Recent);
        s.cycle_mode(true);
        assert_eq!(s.mode, FindMode::Library);
        s.set_query("x".into());
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
}
