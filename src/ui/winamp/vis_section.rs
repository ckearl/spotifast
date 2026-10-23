//! VIS: a wide spectrum analyser under the playlist, Winamp's seventy-five
//! semitone columns each drawn as a bar of its own, in the skin's
//! `viscolor.txt` colours from the bottom up, with falling peak caps.
//!
//! It reads the same tap as the display's analyser, so it shows the sound
//! after the equalizer and before the volume, and moves at zero volume.
//! When the music plays on another device, nothing reaches the tap: the
//! graph lies flat and says where the music is.

use std::time::Instant;

use egui::{Color32, pos2};

use crate::app::{App, NowPlaying};
use crate::i18n::{Locale, gettext};
use crate::model::WinampSection;
use crate::skin::VisColors;
use crate::skin::layout::Area;
use crate::vis::{self, WIDE_BANDS};

use super::View;
use super::section;

/// The spectrum's colours in `viscolor.txt`: sixteen, from the top row
/// down, the same rows the display's analyser paints.
const TOP_COLOR: usize = 2;
const BOTTOM_COLOR: usize = 17;
const PEAK_COLOR: usize = 23;

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// How many of `rows` rows a bar `height` tall (0 to 1) lights.
fn lit(height: f32, rows: u32) -> u32 {
    ((height.clamp(0.0, 1.0) * rows as f32).round() as u32).min(rows)
}

/// The colour of row `row`, counted from the bottom, of a bar `height`
/// tall, from 0 to 1, in a graph `rows` rows tall: the skin's sixteen
/// analyser colours stretched over the rows as the display's analyser
/// paints its sixteen, the bottom colour lowest; transparent above the bar.
pub(super) fn bar_color(colors: &VisColors, height: f32, row: u32, rows: u32) -> Color32 {
    if rows == 0 || row >= lit(height, rows) {
        return Color32::TRANSPARENT;
    }
    let shades = (BOTTOM_COLOR - TOP_COLOR + 1) as u32;
    let step = (row * shades / rows).min(shades - 1);
    rgb(colors[BOTTOM_COLOR - step as usize])
}

/// Where the bars stand across a graph `width` pixels wide.
#[derive(Debug, PartialEq, Eq)]
struct Columns {
    /// The first bar's left edge, from the graph's.
    left: u32,
    /// Each bar's width; one pixel always separates two.
    bar: u32,
    /// From one bar's left edge to the next's.
    pitch: u32,
}

impl Columns {
    /// The bars, as wide as fit whole with a pixel between each, and the
    /// lot centred.
    fn fit(width: u32) -> Self {
        let pitch = (width / WIDE_BANDS as u32).max(2);
        let bar = pitch - 1;
        let span = WIDE_BANDS as u32 * pitch - 1;
        Self {
            left: width.saturating_sub(span) / 2,
            bar,
            pitch,
        }
    }

    fn span(&self) -> u32 {
        WIDE_BANDS as u32 * self.pitch - 1
    }

    fn x(&self, band: usize) -> u32 {
        self.left + band as u32 * self.pitch
    }
}

/// What the graph says in place of bars when the music plays elsewhere.
fn elsewhere(now: &NowPlaying, locale: Locale) -> Option<String> {
    if now.local {
        return None;
    }
    Some(match now.device_name.as_deref() {
        // Translators: {device} is the name of a speaker, phone or computer.
        Some(device) => gettext(locale, "Playing on {device}").replace("{device}", device),
        None => gettext(locale, "Playing on another device").into_owned(),
    })
}

/// The section, under the sections above it. `focused` is the window's.
/// Returns whether the bars are still moving, for the window to keep
/// drawing at the visualiser's rate.
pub(super) fn show(
    app: &mut App,
    view: &mut View,
    now: Option<&NowPlaying>,
    focused: bool,
) -> bool {
    let Some(chrome) = section::frame(app, view, WinampSection::Vis, "VIS", focused) else {
        return false;
    };
    let area = section::body_area(WinampSection::Vis, chrome.height);
    if area.height == 0 {
        return false;
    }
    let colors = view.skin.vis_colors;
    let columns = Columns::fit(area.width);
    let bottom = area.y + area.height - 1;

    if let Some(message) = now.and_then(|now| elsewhere(now, app.locale)) {
        // Whatever was moving starts from rest when the music comes back.
        if !app.winamp.wide.settled() {
            app.winamp.wide = vis::WideAnalyser::default();
        }
        view.fill(
            area.x + columns.left,
            bottom,
            columns.span(),
            1,
            bar_color(&colors, 1.0, 0, area.height),
        );
        let text_area = Area::new(area.x, area.y, area.width, area.height - 1);
        write_centred(app, view, &message, text_area);
        return false;
    }

    let sounding = now.is_some_and(|now| (now.playing || now.loading) && now.local);
    if !sounding && app.winamp.wide.settled() {
        return false;
    }
    let samples = if sounding {
        app.winamp.tap.window(vis::WIDE_SAMPLES, vis::LAG)
    } else {
        Vec::new()
    };
    let levels = app.winamp.wide.step(&samples, Instant::now());
    let peaks = app.winamp.wide.peaks();
    let rows = area.height;
    for (band, (&level, &peak)) in levels.iter().zip(&peaks).enumerate() {
        let x = area.x + columns.x(band);
        for row in 0..lit(level, rows) {
            let color = bar_color(&colors, level, row, rows);
            view.fill(x, bottom - row, columns.bar, 1, color);
        }
        let cap = lit(peak, rows);
        if cap > 0 {
            view.fill(x, bottom + 1 - cap, columns.bar, 1, rgb(colors[PEAK_COLOR]));
        }
    }
    sounding || !app.winamp.wide.settled()
}

/// `text` in the list's font and normal colour, centred in `area`.
fn write_centred(app: &mut App, view: &mut View, text: &str, area: Area) {
    let ctx = view.ui.ctx().clone();
    let pad = 3;
    let room = area.width.saturating_sub(2 * pad);
    let text_cache = &mut app.winamp.playlist_text;
    let label = section::fit(text_cache, text, room as f32);
    let line = text_cache.line(&ctx, &label);
    let (texture, full, height) = (line.texture.id(), line.width, line.height);
    let width = full.min(room);
    let at = Area::new(
        area.x + (area.width - width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height.min(area.height),
    );
    let uv = egui::Rect::from_min_max(
        pos2(0.0, 0.0),
        pos2(
            width as f32 / full.max(1) as f32,
            at.height as f32 / height.max(1) as f32,
        ),
    );
    let painter = view
        .ui
        .painter()
        .with_clip_rect(view.rect(area).intersect(view.ui.clip_rect()));
    painter.image(texture, view.rect(at), uv, rgb(view.skin.playlist.normal));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::layout;

    fn rgb([r, g, b]: [u8; 3]) -> Color32 {
        Color32::from_rgb(r, g, b)
    }

    #[test]
    fn bars_take_the_skins_analyser_colours_bottom_to_top() {
        let c = crate::skin::config::DEFAULT_VIS_COLORS;
        assert_eq!(bar_color(&c, 1.0, 0, 16), rgb(c[17]));
        assert_eq!(bar_color(&c, 1.0, 15, 16), rgb(c[2]));
    }

    /// The section's graph is 29 rows: the sixteen colours stretch over
    /// them, bottom to top, none skipped and none out of order.
    #[test]
    fn a_taller_graph_stretches_all_sixteen_colours() {
        let c = crate::skin::config::DEFAULT_VIS_COLORS;
        let rows = layout::PLAYLIST_TILE_HEIGHT;
        assert_eq!(rows, 29);
        assert_eq!(bar_color(&c, 1.0, 0, rows), rgb(c[17]));
        assert_eq!(bar_color(&c, 1.0, rows - 1, rows), rgb(c[2]));
        let shades: Vec<Color32> = (0..rows).map(|row| bar_color(&c, 1.0, row, rows)).collect();
        let mut distinct = shades.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), 16);
        assert_eq!(
            distinct,
            (2..=17).rev().map(|i| rgb(c[i])).collect::<Vec<_>>()
        );
    }

    /// A row keeps its colour whatever the bar's height, as Winamp's
    /// analyser paints, and there is nothing above the bar.
    #[test]
    fn a_short_bar_is_its_lower_rows_and_nothing_above() {
        let c = crate::skin::config::DEFAULT_VIS_COLORS;
        assert_eq!(bar_color(&c, 0.5, 0, 16), rgb(c[17]));
        assert_eq!(bar_color(&c, 0.5, 7, 16), bar_color(&c, 1.0, 7, 16));
        assert_eq!(bar_color(&c, 0.5, 8, 16), Color32::TRANSPARENT);
        assert_eq!(bar_color(&c, 0.0, 0, 16), Color32::TRANSPARENT);
    }

    /// Seventy-five bars with a pixel between each fill the 243-pixel
    /// list as wide as whole pixels allow, centred.
    #[test]
    fn the_bars_fit_the_list_width_centred() {
        let width = section::body_area(WinampSection::Vis, layout::VIS_HEIGHT).width;
        assert_eq!(width, 243);
        let columns = Columns::fit(width);
        assert_eq!(
            columns,
            Columns {
                left: 9,
                bar: 2,
                pitch: 3
            }
        );
        assert_eq!(columns.span(), 224);
        let right = width - (columns.left + columns.span());
        assert!(right.abs_diff(columns.left) <= 1);
        assert_eq!(
            columns.x(WIDE_BANDS - 1) + columns.bar,
            columns.left + columns.span()
        );
        // Narrower than three pixels a band, the bars are a pixel each.
        assert_eq!(
            Columns::fit(149),
            Columns {
                left: 0,
                bar: 1,
                pitch: 2
            }
        );
    }

    fn now(local: bool, device: Option<&str>) -> NowPlaying {
        let mut now = super::super::tests::now("Song", "Artist", 1000);
        now.local = local;
        now.device_name = device.map(str::to_string);
        now
    }

    #[test]
    fn music_elsewhere_names_the_device() {
        let locale = Locale::default();
        assert_eq!(elsewhere(&now(true, None), locale), None);
        assert_eq!(
            elsewhere(&now(false, Some("Kitchen")), locale).as_deref(),
            Some("Playing on Kitchen")
        );
        assert_eq!(
            elsewhere(&now(false, None), locale).as_deref(),
            Some("Playing on another device")
        );
    }
}
