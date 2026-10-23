//! The frame FIND, LYRICS and VIS hang in, under the playlist.
//!
//! Classic skins have no art for these sections, so each is built from the
//! playlist's own pieces of `pledit.bmp`: its corners, with title tiles all
//! the way across so the baked-in name never shows, and the section's name
//! written over them in the skin's font; its side tiles; and its blank
//! bottom tile, capped at each end by the edges of the playlist's bottom
//! bar so the window keeps its border, with the section's own buttons
//! written on it. Rolled up, a section is the playlist's shade bar with its
//! name. The list inside scrolls the way the playlist's does.

use std::borrow::Cow;

use egui::{Color32, Sense};

use crate::app::App;
use crate::model::{Action, WinampSection};
use crate::skin::layout::{self, Area};
use crate::skin::{Sprite, sprites};

use super::{PixelText, View};

/// A section drawn open, for its body to fill.
pub(super) struct Chrome {
    pub height: u32,
}

/// The name a section's widgets are known by.
fn key(section: WinampSection) -> &'static str {
    match section {
        WinampSection::Find => "find",
        WinampSection::Lyrics => "lyrics",
        WinampSection::Vis => "vis",
    }
}

/// VIS has one height; the others stretch like the playlist.
fn resizable(section: WinampSection) -> bool {
    section != WinampSection::Vis
}

/// The bottom bar: the playlist's whole bar, or for VIS, which has no
/// buttons, the short one.
fn bottom_height(section: WinampSection) -> u32 {
    match section {
        WinampSection::Vis => layout::SECTION_SHORT_BOTTOM_HEIGHT,
        _ => layout::PLAYLIST_BOTTOM_HEIGHT,
    }
}

/// What a section has inside its frame, `height` skin pixels tall.
pub(super) fn body_area(section: WinampSection, height: u32) -> Area {
    Area::new(
        layout::PLAYLIST_LEFT_WIDTH,
        layout::PLAYLIST_TITLE_HEIGHT,
        layout::WINDOW_WIDTH - layout::PLAYLIST_LEFT_WIDTH - layout::PLAYLIST_RIGHT_WIDTH,
        height.saturating_sub(layout::PLAYLIST_TITLE_HEIGHT + bottom_height(section)),
    )
}

/// The list's area in a section with the playlist's bottom bar: the same
/// as the playlist's own.
pub(super) fn list_area(height: u32) -> Area {
    body_area(WinampSection::Find, height)
}

/// The section, rolled up or open. Rolled up, it draws the shade bar and
/// returns `None`; open, it draws the frame and an empty body, wires the
/// title bar, its buttons and, when the section stretches, the grip, and
/// returns how tall it is.
pub(super) fn frame(
    app: &mut App,
    view: &mut View,
    section: WinampSection,
    title: &str,
    focused: bool,
) -> Option<Chrome> {
    if app.settings.section_shaded(section) {
        shade(app, view, section, title, focused);
        return None;
    }
    let height = app.settings.section_height(section);
    let name = key(section);
    top(view, title, focused);
    sides(view, section, height);
    bottom(view, section, height);
    let body = body_area(section, height);
    let [r, g, b] = view.skin.playlist.normal_background;
    view.fill(
        body.x,
        body.y,
        body.width,
        body.height,
        Color32::from_rgb(r, g, b),
    );

    let bar = view.interact(
        Area::new(0, 0, layout::WINDOW_WIDTH, layout::PLAYLIST_TITLE_HEIGHT),
        &format!("{name}-title"),
        Sense::click_and_drag(),
    );
    if bar.drag_started() {
        view.ui
            .ctx()
            .send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if view
        .lamp_button(
            layout::PLAYLIST_CLOSE,
            sprites::PLAYLIST_CLOSE_PRESSED,
            false,
            &format!("{name}-close"),
        )
        .clicked()
    {
        app.actions.push(Action::ToggleWinampSection(section));
    }
    if view
        .lamp_button(
            layout::PLAYLIST_SHADE,
            sprites::PLAYLIST_SHADE_PRESSED,
            false,
            &format!("{name}-shade"),
        )
        .clicked()
    {
        app.actions.push(Action::ToggleWinampSectionShade(section));
    }
    if resizable(section) {
        grip(app, view, section, height);
    }
    Some(Chrome { height })
}

/// A piece of a sprite: `width` by `height` from `dx`, `dy` in.
fn cut(sprite: Sprite, dx: u32, dy: u32, width: u32, height: u32) -> Sprite {
    Sprite::new(sprite.sheet, sprite.x + dx, sprite.y + dy, width, height)
}

/// The title bar: the corners, title tiles between them where the
/// playlist has its name, and the section's name on them.
fn top(view: &View, title: &str, focused: bool) {
    use sprites::*;
    let (left, tile, right) = if focused {
        (
            PLAYLIST_TOP_LEFT_ACTIVE,
            PLAYLIST_TOP_TILE_ACTIVE,
            PLAYLIST_TOP_RIGHT_ACTIVE,
        )
    } else {
        (PLAYLIST_TOP_LEFT, PLAYLIST_TOP_TILE, PLAYLIST_TOP_RIGHT)
    };
    let width = layout::WINDOW_WIDTH;
    let corner = layout::PLAYLIST_TILE_WIDTH;
    view.sprite_at(left, 0, 0);
    let run = Area::new(corner, 0, width - 2 * corner, layout::PLAYLIST_TITLE_HEIGHT);
    let mut x = corner;
    while x < width - corner {
        view.sprite_clipped(tile, x, 0, run);
        x += corner;
    }
    view.sprite_at(right, width - corner, 0);
    // A blank cell either side, so the name sits in a gap in the stripes
    // the way the playlist's does.
    let label = format!(" {title} ");
    let room = (run.width / 5) as usize;
    let label: String = label.chars().take(room).collect();
    let label_width = 5 * label.chars().count() as u32;
    view.text(
        &label,
        Area::new(
            (width - label_width) / 2,
            (layout::PLAYLIST_TITLE_HEIGHT - 6) / 2,
            label_width,
            6,
        ),
    );
}

/// The side tiles, down to the bottom bar.
fn sides(view: &View, section: WinampSection, height: u32) {
    let middle = Area::new(
        0,
        layout::PLAYLIST_TITLE_HEIGHT,
        layout::WINDOW_WIDTH,
        height.saturating_sub(layout::PLAYLIST_TITLE_HEIGHT + bottom_height(section)),
    );
    let mut y = middle.y;
    while y < middle.y + middle.height {
        view.sprite_clipped(sprites::PLAYLIST_LEFT_TILE, 0, y, middle);
        view.sprite_clipped(
            sprites::PLAYLIST_RIGHT_TILE,
            layout::WINDOW_WIDTH - layout::PLAYLIST_RIGHT_WIDTH,
            y,
            middle,
        );
        y += layout::PLAYLIST_TILE_HEIGHT;
    }
}

/// Rows kept from the foot of a bottom piece when the bar is cut short:
/// its lower edge.
const SHORT_BAR_FOOT: u32 = 2;

/// The bottom bar: the blank tile across, then the left edge of the
/// playlist's bar and the right end of it, where the grip is. Those ends
/// are clear of the playlist's buttons in Winamp's layout, which puts ADD
/// at 14 and LIST OPTS at 231.
fn bottom(view: &View, section: WinampSection, height: u32) {
    use sprites::*;
    let bar = bottom_height(section);
    let y = height - bar;
    let full = layout::PLAYLIST_BOTTOM_HEIGHT;
    let width = layout::WINDOW_WIDTH;
    let clip = Area::new(0, y, width, bar);
    // The piece whole, or its top and its foot when the bar is short.
    let piece = |sprite: Sprite, x: u32| {
        if bar >= full {
            view.sprite_clipped(sprite, x, y, clip);
            return;
        }
        let foot = SHORT_BAR_FOOT.min(bar);
        view.sprite_clipped(cut(sprite, 0, 0, sprite.width, bar - foot), x, y, clip);
        view.sprite_clipped(
            cut(sprite, 0, full - foot, sprite.width, foot),
            x,
            y + bar - foot,
            clip,
        );
    };
    let mut x = 0;
    while x < width {
        piece(PLAYLIST_BOTTOM_TILE, x);
        x += layout::PLAYLIST_TILE_WIDTH;
    }
    piece(
        cut(
            PLAYLIST_BOTTOM_LEFT,
            0,
            0,
            layout::PLAYLIST_LEFT_WIDTH,
            full,
        ),
        0,
    );
    piece(
        cut(
            PLAYLIST_BOTTOM_RIGHT,
            PLAYLIST_BOTTOM_RIGHT.width - layout::PLAYLIST_RIGHT_WIDTH,
            0,
            layout::PLAYLIST_RIGHT_WIDTH,
            full,
        ),
        width - layout::PLAYLIST_RIGHT_WIDTH,
    );
}

/// The section rolled up: the playlist's shade bar with the section's
/// name, the button to roll it down again, and its X.
fn shade(app: &mut App, view: &mut View, section: WinampSection, title: &str, focused: bool) {
    use sprites::*;
    let name = key(section);
    let width = layout::WINDOW_WIDTH;
    let height = layout::SECTION_SHADE_HEIGHT;
    view.sprite_at(PLAYLIST_SHADE_LEFT, 0, 0);
    let mut x = 25;
    while x < width - 50 {
        view.sprite_clipped(
            PLAYLIST_SHADE_TILE,
            x,
            0,
            Area::new(25, 0, width - 75, height),
        );
        x += 25;
    }
    let right = if focused {
        PLAYLIST_SHADE_RIGHT_ACTIVE
    } else {
        PLAYLIST_SHADE_RIGHT
    };
    view.sprite_at(right, width - 50, 0);
    let bar = view.interact(
        Area::new(0, 0, width, height),
        &format!("{name}-shade-title"),
        Sense::click_and_drag(),
    );
    if bar.drag_started() {
        view.ui
            .ctx()
            .send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    view.text(title, Area::new(5, 4, width - 30 - 5 - 5, 6));
    if view
        .lamp_button(
            layout::PLAYLIST_SHADE,
            PLAYLIST_UNSHADE_PRESSED,
            false,
            &format!("{name}-unshade"),
        )
        .clicked()
    {
        app.actions.push(Action::ToggleWinampSectionShade(section));
    }
    if view
        .lamp_button(
            layout::PLAYLIST_CLOSE,
            PLAYLIST_CLOSE_PRESSED,
            false,
            &format!("{name}-shade-close"),
        )
        .clicked()
    {
        app.actions.push(Action::ToggleWinampSection(section));
    }
}

/// A height moved by whole steps of the grip, kept in range.
fn resized(height: u32, steps: i64) -> u32 {
    (height as i64 + steps * layout::SECTION_RESIZE_STEP as i64).clamp(
        layout::SECTION_MIN_HEIGHT as i64,
        layout::SECTION_MAX_HEIGHT as i64,
    ) as u32
}

/// The corner that stretches the section, a tile row at a time.
fn grip(app: &mut App, view: &mut View, section: WinampSection, height: u32) {
    let corner = Area::new(
        layout::WINDOW_WIDTH - layout::PLAYLIST_GRIP,
        height - layout::PLAYLIST_GRIP,
        layout::PLAYLIST_GRIP,
        layout::PLAYLIST_GRIP,
    );
    let response = view
        .interact(corner, &format!("{}-grip", key(section)), Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if response.dragged() {
        app.winamp.section_resize += response.drag_delta().y / view.unit;
        let step = layout::SECTION_RESIZE_STEP as f32;
        let steps = (app.winamp.section_resize / step).trunc();
        if steps != 0.0 {
            app.winamp.section_resize -= steps * step;
            let wanted = resized(height, steps as i64);
            if wanted != height {
                app.actions
                    .push(Action::SetWinampSectionHeight(section, wanted));
            }
        }
    }
    if response.drag_stopped() {
        app.winamp.section_resize = 0.0;
    }
}

/// Padding inside a bottom button, and the gap between two.
const BUTTON_PAD: u32 = 4;
/// Where the buttons sit in the bottom bar, from its top: the height of
/// the playlist's menu buttons, with the label centred in the bar.
const BUTTON_TOP: u32 = 10;
const BUTTON_HEIGHT: u32 = 18;

/// Where each bottom button goes, from the left edge of the list, its y
/// from the top of the bottom bar.
fn button_spans(labels: &[&str]) -> Vec<Area> {
    let mut x = layout::PLAYLIST_LEFT_WIDTH;
    labels
        .iter()
        .map(|label| {
            let width = 5 * label.chars().count() as u32 + 2 * BUTTON_PAD;
            let span = Area::new(x, BUTTON_TOP, width, BUTTON_HEIGHT);
            x += width + BUTTON_PAD;
            span
        })
        .collect()
}

/// Buttons along the bottom bar, their labels in the skin's font, a label
/// a pixel lower while held. Returns the one clicked this frame.
#[expect(
    dead_code,
    reason = "FIND draws its buttons with this from the next change on"
)]
pub(super) fn bottom_buttons(
    view: &mut View,
    height: u32,
    labels: &[&str],
    id: &str,
) -> Option<usize> {
    let bar = height - layout::PLAYLIST_BOTTOM_HEIGHT;
    let mut clicked = None;
    for (index, (label, span)) in labels.iter().zip(button_spans(labels)).enumerate() {
        let area = Area::new(span.x, bar + span.y, span.width, span.height);
        let response = view.interact(area, &format!("{id}-button-{index}"), Sense::click());
        let held = u32::from(response.is_pointer_button_down_on());
        view.text(
            label,
            Area::new(
                area.x + BUTTON_PAD,
                area.y + (area.height - 6) / 2 + held,
                area.width - 2 * BUTTON_PAD,
                6,
            ),
        );
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// One line of a section's list.
pub(super) struct ListRow<'a> {
    pub label: Cow<'a, str>,
    /// Written against the right edge, as the playlist writes a time.
    pub right: Option<String>,
    pub current: bool,
    pub selected: bool,
}

/// What happened to the list this frame, by row.
pub(super) struct ListResponse {
    pub clicked: Option<usize>,
    pub double_clicked: Option<usize>,
    pub secondary: Option<usize>,
}

/// The rows, in the skin's colours and the playlist's font, scrolled by
/// the wheel and by the handle in the right-hand tiles. `scroll` is the
/// first row in view and `wheel` the wheel's leftover, both kept by the
/// caller between frames; middle-click scrolling writes the section's own
/// scroll in `WinampState` after the frame.
pub(super) fn list(
    view: &mut View,
    text: &mut PixelText,
    rows: &[ListRow],
    scroll: &mut usize,
    wheel: &mut f32,
    area: Area,
    section: WinampSection,
) -> ListResponse {
    let id = key(section);
    let style = view.skin.playlist.clone();
    let rgb = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
    view.fill(
        area.x,
        area.y,
        area.width,
        area.height,
        rgb(style.normal_background),
    );
    let mut response = ListResponse {
        clicked: None,
        double_clicked: None,
        secondary: None,
    };

    let visible = (area.height / layout::PLAYLIST_TRACK_HEIGHT) as usize;
    let most = rows.len().saturating_sub(visible);
    // `contains_pointer`, not `hovered`: the rows drawn on top of the list
    // would take the hover and the wheel with it.
    if view
        .interact(area, &format!("{id}-list"), Sense::hover())
        .contains_pointer()
    {
        *wheel += wheel_rows(view, visible);
        let moved = wheel.trunc();
        if moved != 0.0 {
            *wheel -= moved;
            *scroll = (*scroll as i64 + moved as i64).clamp(0, most as i64) as usize;
        }
    }
    *scroll = (*scroll).min(most);
    let row_points = layout::PLAYLIST_TRACK_HEIGHT as f32 * view.unit;
    let rect = view.rect(area);
    // Middle-click scrolling hands its rows back to this section's list.
    crate::autoscroll::playlist(view.ui, rect, *scroll, most, row_points, Some(section));

    let ctx = view.ui.ctx().clone();
    let painter = view
        .ui
        .painter()
        .with_clip_rect(rect.intersect(view.ui.clip_rect()));
    let pad = 3;
    for (offset, row) in rows.iter().skip(*scroll).take(visible).enumerate() {
        let index = *scroll + offset;
        let line = Area::new(
            area.x,
            area.y + offset as u32 * layout::PLAYLIST_TRACK_HEIGHT,
            area.width,
            layout::PLAYLIST_TRACK_HEIGHT,
        );
        let rect = view.rect(line);
        let clicks = view.ui.interact(
            rect,
            egui::Id::new(("winamp-section-row", id, index)),
            Sense::click(),
        );
        crate::autoscroll::row(view.ui, &clicks);
        if clicks.clicked() {
            response.clicked = Some(index);
        }
        if clicks.double_clicked() {
            response.double_clicked = Some(index);
        }
        if clicks.secondary_clicked() {
            response.secondary = Some(index);
        }
        if row.selected {
            painter.rect_filled(rect, 0.0, rgb(style.selected_background));
        }
        let color = rgb(if row.current {
            style.current
        } else {
            style.normal
        });
        let mut room = line.width.saturating_sub(2 * pad);
        if let Some(right) = &row.right {
            let right_line = text.line(&ctx, right);
            let at = Area::new(
                line.x + line.width - pad - right_line.width,
                line.y + line.height.saturating_sub(right_line.height) / 2,
                right_line.width,
                right_line.height,
            );
            painter.image(
                right_line.texture.id(),
                view.rect(at),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                color,
            );
            room = room.saturating_sub(pad + at.width);
        }
        let label = fit(text, &row.label, room as f32);
        let label_line = text.line(&ctx, &label);
        let at = Area::new(
            line.x + pad,
            line.y + line.height.saturating_sub(label_line.height) / 2,
            label_line.width.min(room),
            label_line.height,
        );
        let uv_right = at.width as f32 / label_line.width.max(1) as f32;
        painter.image(
            label_line.texture.id(),
            view.rect(at),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(uv_right, 1.0)),
            color,
        );
    }
    scroll_handle(view, scroll, most, area, id);
    response
}

/// The handle in the right-hand tiles beside the list, dragged to scroll.
fn scroll_handle(view: &mut View, scroll: &mut usize, most: usize, area: Area, id: &str) {
    let travel = area
        .height
        .saturating_sub(layout::PLAYLIST_SCROLL_HANDLE_HEIGHT);
    let fraction = if most == 0 {
        0.0
    } else {
        *scroll as f32 / most as f32
    };
    let handle = Area::new(
        layout::PLAYLIST_SCROLL_X,
        area.y + (fraction * travel as f32).round() as u32,
        8,
        layout::PLAYLIST_SCROLL_HANDLE_HEIGHT,
    );
    let response = view.interact(handle, &format!("{id}-scroll"), Sense::click_and_drag());
    if response.dragged()
        && most > 0
        && let Some(pos) = response.interact_pointer_pos()
    {
        let pointer = (pos.y - view.origin.y) / view.unit
            - area.y as f32
            - layout::PLAYLIST_SCROLL_HANDLE_HEIGHT as f32 / 2.0;
        let fraction = (pointer / travel.max(1) as f32).clamp(0.0, 1.0);
        *scroll = (fraction * most as f32).round() as usize;
    }
    let sprite = if response.dragged() || response.is_pointer_button_down_on() {
        sprites::PLAYLIST_SCROLL_HANDLE_PRESSED
    } else {
        sprites::PLAYLIST_SCROLL_HANDLE
    };
    view.sprite(sprite, handle);
}

/// How many rows this frame's wheel moves a list of `visible` rows,
/// positive downwards.
pub(super) fn wheel_rows(view: &View, visible: usize) -> f32 {
    let row_points = layout::PLAYLIST_TRACK_HEIGHT as f32 * view.unit;
    let (lines, points, pages) = view.ui.ctx().input(|input| {
        input
            .events
            .iter()
            .fold((0.0, 0.0, 0.0), |sum, event| match event {
                egui::Event::MouseWheel { unit, delta, .. } => match unit {
                    // One detent is one event however many lines the
                    // system says it is worth; a free-spinning wheel's
                    // fractions still add up.
                    egui::MouseWheelUnit::Line => {
                        let notch = if delta.y.abs() >= 1.0 {
                            delta.y.signum()
                        } else {
                            delta.y
                        };
                        (sum.0 + notch, sum.1, sum.2)
                    }
                    egui::MouseWheelUnit::Point => (sum.0, sum.1 + delta.y, sum.2),
                    egui::MouseWheelUnit::Page => (sum.0, sum.1, sum.2 + delta.y),
                },
                _ => sum,
            })
    });
    rows_for_wheel(lines, points, pages, row_points, visible)
}

/// How many rows a frame's wheel moves the list, positive downwards. A
/// notch scrolls three rows, the Windows default Winamp's list followed;
/// a trackpad's points scroll a row per row's height; a page is the rows
/// in view. egui's deltas are positive when the content moves down, which
/// is scrolling up.
pub(super) fn rows_for_wheel(
    lines: f32,
    points: f32,
    pages: f32,
    row_points: f32,
    visible: usize,
) -> f32 {
    -(lines * 3.0 + points / row_points + pages * visible as f32)
}

/// Text cut to a width, with an ellipsis when it had to be.
pub(super) fn fit(text: &mut PixelText, label: &str, width: f32) -> String {
    if text.width(label) <= width {
        return label.to_string();
    }
    let chars: Vec<char> = label.chars().collect();
    let mut keep = chars.len();
    while keep > 0 {
        keep -= 1;
        let candidate: String = chars[..keep].iter().collect::<String>() + "\u{2026}";
        if text.width(&candidate) <= width {
            return candidate;
        }
    }
    "\u{2026}".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_list_holds_whole_rows() {
        assert_eq!(list_area(174), Area::new(12, 20, 243, 116));
        assert_eq!(list_area(116).height / layout::PLAYLIST_TRACK_HEIGHT, 4);
    }

    #[test]
    fn bottom_buttons_share_the_bar_without_overlap() {
        let spans = button_spans(&["PLAY", "+QUEUE", "+LIST", "RADIO", "BACK"]);
        assert!(spans.windows(2).all(|w| w[0].x + w[0].width < w[1].x));
        assert!(
            spans.last().unwrap().x + spans.last().unwrap().width
                <= layout::WINDOW_WIDTH - layout::PLAYLIST_RIGHT_WIDTH
        );
    }

    /// VIS is as tall as the playlist's title and bottom bars together, so
    /// its bar is a short one, and what is left is one row of side tiles.
    #[test]
    fn the_vis_section_keeps_one_tile_row_for_its_graph() {
        let body = body_area(WinampSection::Vis, layout::VIS_HEIGHT);
        assert_eq!(body, Area::new(12, 20, 243, layout::PLAYLIST_TILE_HEIGHT));
        assert_eq!(
            body_area(WinampSection::Find, 174),
            list_area(174),
            "FIND and LYRICS keep the playlist's geometry"
        );
    }

    #[test]
    fn the_grip_moves_in_whole_steps_within_the_range() {
        assert_eq!(resized(116, 2), 174);
        assert_eq!(resized(145, -1), 116);
        assert_eq!(resized(116, -3), layout::SECTION_MIN_HEIGHT);
        assert_eq!(
            resized(layout::SECTION_MAX_HEIGHT, 4),
            layout::SECTION_MAX_HEIGHT
        );
    }
}
