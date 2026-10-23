//! DEV's menu: where the music plays, this computer or a Spotify Connect
//! device, listed as the big window's picker lists them.

use std::time::Instant;

use egui::{Ui, vec2};

use crate::app::App;
use crate::i18n::gettext;
use crate::model::Action;
use crate::ui::devices::{self as picker, OutputRow};

/// The key of the row that sets up playback on this computer.
const SET_UP_HERE: &str = "set-up-playback-here";

/// One row of the menu.
pub(super) struct Entry {
    /// What [`choose`] is given: a device's id, or a receiver's key.
    pub key: String,
    pub label: String,
    /// The output playing now.
    pub checked: bool,
    /// Spotify marks it restricted: greyed, though it can still be chosen,
    /// as in the big window, which drives it over Connect.
    pub restricted: bool,
    /// What the big window says under the name, for the pointer's hint.
    pub status: String,
    action: Option<Action>,
    /// What the marquee says once it is chosen.
    announcement: Option<String>,
}

/// The menu's rows, in the big window's picker's order and by its rules:
/// a row to set up playback here while it is not, then the output playing
/// now, then the others by how often they are chosen.
pub(super) fn entries(app: &App) -> Vec<Entry> {
    let locale = app.locale;
    let this_computer = gettext(locale, "This computer").into_owned();
    let mut entries = Vec::new();
    if !app.local_ready {
        let setting_up = matches!(
            app.local_playback,
            crate::backend::LocalPlayback::Authorizing
                | crate::backend::LocalPlayback::Connecting
                | crate::backend::LocalPlayback::WaitingForNetwork
        );
        entries.push(Entry {
            key: SET_UP_HERE.into(),
            label: this_computer.clone(),
            checked: false,
            restricted: false,
            status: if setting_up {
                gettext(locale, "Setting up…")
            } else {
                gettext(locale, "Set up playback here")
            }
            .into_owned(),
            action: (!setting_up).then_some(Action::EnablePlayback),
            announcement: None,
        });
    }
    let outputs = picker::outputs(app);
    let active_id = outputs.active_id.as_deref();
    for row in &outputs.rows {
        let action = picker::choice(app, row, active_id);
        entries.push(match row {
            OutputRow::Device(device) => {
                let is_local = device.id.is_some() && device.id == outputs.local_id;
                let checked = device.id.is_some() && device.id.as_deref() == active_id;
                Entry {
                    key: device.id.clone().unwrap_or_else(|| row.widget_key()),
                    label: if is_local {
                        this_computer.clone()
                    } else {
                        device.name.clone()
                    },
                    checked,
                    restricted: device.is_restricted,
                    status: if checked {
                        gettext(locale, "Listening on this device").into_owned()
                    } else if device.is_restricted {
                        gettext(locale, "Restricted").into_owned()
                    } else if is_local {
                        gettext(locale, "Play here").into_owned()
                    } else {
                        device.kind.replace('_', " ")
                    },
                    action,
                    announcement: Some(if is_local {
                        gettext(locale, "Playing on this computer").into_owned()
                    } else {
                        playing_on(app, &device.name)
                    }),
                }
            }
            OutputRow::Receiver(receiver) => Entry {
                key: row.widget_key(),
                label: receiver.name.clone(),
                checked: false,
                restricted: false,
                status: if action.is_none() {
                    gettext(locale, "Connecting…")
                } else {
                    gettext(locale, "On your network, click to connect")
                }
                .into_owned(),
                action,
                announcement: Some(playing_on(app, &receiver.name)),
            },
        });
    }
    entries
}

fn playing_on(app: &App, name: &str) -> String {
    // Translators: {device} is the name of a speaker, phone or computer.
    gettext(app.locale, "Playing on {device}").replace("{device}", name)
}

/// Does what choosing the row in the big window does, and has the marquee
/// say where the music now plays. The row playing now, or one already
/// connecting, asks for nothing.
pub(super) fn choose(app: &mut App, key: &str) {
    let Some(entry) = entries(app).into_iter().find(|entry| entry.key == key) else {
        return;
    };
    let Some(action) = entry.action else {
        return;
    };
    if let Some(text) = entry.announcement {
        app.winamp.announce(text, Instant::now());
    }
    app.actions.push(action);
}

/// DEV's menu, under `button`: open on a click, or on the frame after
/// `g d` asks; asking Spotify and the network for devices as it opens.
pub(super) fn popup(app: &mut App, skin: &crate::skin::Skin, unit: f32, button: &egui::Response) {
    let ctx = button.ctx.clone();
    let id = egui::Popup::default_response_id(button);
    let was_open = egui::Popup::is_id_open(&ctx, id);
    if std::mem::take(&mut app.winamp.open_devices) {
        egui::Popup::open_id(&ctx, id);
    }
    // DEV sits at the window's foot, so the menu opens upwards and
    // scrolls in the room above it.
    let room = (button.rect.top() - ctx.content_rect().top() - 4.0 * unit.max(1.0)).max(unit);
    let popup = egui::Popup::menu(button)
        .align(egui::RectAlign::TOP_START)
        .align_alternatives(&[egui::RectAlign::BOTTOM_START]);
    let shown = super::menu(popup, skin, unit, |ui| {
        // Grow past the popup's remembered height, as the big window's
        // picker does; a short list still shrinks to its rows.
        egui::ScrollArea::vertical()
            .min_scrolled_height(room)
            .max_height(room)
            .show(ui, |ui| menu(app, ui));
    });
    if shown.is_some() && !was_open {
        app.actions.push(Action::RefreshDevices);
    }
}

/// The menu's contents, inside `super::menu`'s popup.
pub(super) fn menu(app: &mut App, ui: &mut Ui) {
    let font = ui.text_style_height(&egui::TextStyle::Button);
    ui.set_min_width(font * 12.0);
    let weak = ui.visuals().weak_text_color();
    if app.devices_loading {
        ui.add_enabled(
            false,
            egui::Label::new(gettext(app.locale, "Finding devices…").as_ref()),
        );
    }
    let listed = entries(app);
    if listed.is_empty() {
        ui.add_enabled(
            false,
            egui::Label::new(gettext(app.locale, "No devices found").as_ref()),
        );
    }
    // The check sits in a gutter of its own, so the names line up.
    let gutter = font;
    let width = font * 12.0;
    for entry in listed {
        let mut text = egui::RichText::new(&entry.label);
        if entry.restricted || entry.action.is_none() && !entry.checked {
            text = text.color(weak);
        }
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().button_padding.x += gutter;
                ui.add(
                    egui::Button::new(text)
                        .min_size(vec2(width, 0.0))
                        .selected(false),
                )
            })
            .inner
            .on_hover_text(&entry.status);
        if entry.checked {
            tick(ui, &response, gutter);
        }
        if response.clicked() {
            choose(app, &entry.key);
            ui.close();
        }
    }
}

/// A check mark in the row's gutter, in the row's text colour.
fn tick(ui: &Ui, response: &egui::Response, gutter: f32) {
    let color = ui.style().interact(response).fg_stroke.color;
    let rect = response.rect;
    let size = gutter * 0.6;
    let left = rect.left() + ui.spacing().button_padding.x + (gutter - size) / 2.0;
    let mid = rect.center().y;
    let points = vec![
        egui::pos2(left, mid),
        egui::pos2(left + size * 0.35, mid + size * 0.35),
        egui::pos2(left + size, mid - size * 0.45),
    ];
    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::Device;
    use crate::app::App;
    use crate::model::Action;

    fn test_app() -> App {
        let root = std::env::temp_dir().join(format!(
            "spotifast-winamp-devices-{}-{}",
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

    fn device(id: &str, name: &str, kind: &str) -> Device {
        Device {
            id: Some(id.into()),
            name: name.into(),
            kind: kind.into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_active_device_is_checked_and_choosing_transfers() {
        let mut app = test_app();
        app.local_device_id = Some("local".into());
        app.local_ready = true;
        app.devices = vec![
            device("k", "Kitchen", "speaker"),
            device("local", "Spotifast", "computer"),
        ];
        let listed = entries(&app);
        assert_eq!(
            listed
                .iter()
                .map(|entry| (entry.label.as_str(), entry.checked))
                .collect::<Vec<_>>(),
            [("This computer", true), ("Kitchen", false)]
        );
        app.actions.clear();
        choose(&mut app, "k");
        assert!(
            matches!(app.actions.as_slice(), [Action::Transfer(id)] if id == "k"),
            "{:?}",
            app.actions
        );
        let now = std::time::Instant::now();
        assert_eq!(
            app.winamp.announcement(now).map(|(text, _)| text),
            Some("Playing on Kitchen")
        );
        app.backend.shutdown();
    }

    #[test]
    fn devices_menu_opens_on_g_d_and_asks_for_devices_once() {
        let mut app = test_app();
        app.local_device_id = Some("local".into());
        app.local_ready = true;
        app.devices = vec![device("k", "Kitchen", "speaker")];
        let skin = crate::skin::Skin::builtin();
        let ctx = egui::Context::default();
        let frame = |app: &mut App| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let button = ui.button("DEV");
                popup(app, &skin, 2.0, &button);
            });
            output.textures_delta.clear();
        };
        app.actions.clear();
        frame(&mut app);
        assert!(app.actions.is_empty(), "closed: {:?}", app.actions);
        app.winamp.open_devices = true;
        frame(&mut app);
        assert!(!app.winamp.open_devices, "the ask is taken");
        assert!(
            matches!(app.actions.as_slice(), [Action::RefreshDevices]),
            "{:?}",
            app.actions
        );
        app.actions.clear();
        frame(&mut app);
        assert!(app.actions.is_empty(), "still open: {:?}", app.actions);
        app.backend.shutdown();
    }

    #[test]
    fn devices_choosing_the_playing_one_does_nothing() {
        let mut app = test_app();
        app.local_device_id = Some("local".into());
        app.local_ready = true;
        app.devices = vec![device("local", "Spotifast", "computer")];
        app.actions.clear();
        choose(&mut app, "local");
        assert!(app.actions.is_empty(), "{:?}", app.actions);
        assert!(app.winamp.announcement(std::time::Instant::now()).is_none());
        app.backend.shutdown();
    }

    #[test]
    fn devices_restricted_are_greyed_and_waiting_receivers_activate() {
        let mut app = test_app();
        app.local_device_id = Some("local".into());
        app.local_ready = true;
        app.devices = vec![Device {
            is_restricted: true,
            ..device("sonos", "Den", "speaker")
        }];
        let receiver = crate::zeroconf::Receiver {
            name: "House Spotify".into(),
            device_id: Some("house".into()),
            address: "192.0.2.4".parse().unwrap(),
            port: 5555,
            path: "/zc".into(),
            cast: false,
        };
        app.receivers = vec![receiver.clone()];
        let listed = entries(&app);
        let den = listed.iter().find(|entry| entry.label == "Den").unwrap();
        assert!(den.restricted && !den.checked);
        let house = listed
            .iter()
            .find(|entry| entry.label == "House Spotify")
            .unwrap();
        let key = house.key.clone();
        app.actions.clear();
        choose(&mut app, &key);
        assert!(
            matches!(app.actions.as_slice(), [Action::ActivateReceiver(chosen)] if **chosen == receiver),
            "{:?}",
            app.actions
        );
        app.backend.shutdown();
    }

    #[test]
    fn devices_without_playback_here_offer_to_set_it_up() {
        let mut app = test_app();
        app.local_ready = false;
        app.devices = vec![device("k", "Kitchen", "speaker")];
        let listed = entries(&app);
        assert_eq!(listed[0].label, "This computer");
        let key = listed[0].key.clone();
        app.actions.clear();
        choose(&mut app, &key);
        assert!(
            matches!(app.actions.as_slice(), [Action::EnablePlayback]),
            "{:?}",
            app.actions
        );
        app.backend.shutdown();
    }

    #[test]
    fn devices_announcement_lasts_one_pass_of_the_marquee() {
        let mut state = test_app();
        let start = std::time::Instant::now();
        state.winamp.announce("Playing on Kitchen".into(), start);
        assert!(state.winamp.announcement(start).is_some());
        assert!(
            state
                .winamp
                .announcement(start + std::time::Duration::from_secs(4))
                .is_none()
        );
        let long = "Playing on the long-named speaker in the upstairs bedroom";
        state.winamp.announce(long.into(), start);
        let pass = std::time::Duration::from_millis(220)
            * (long.chars().count() + "  ***  ".chars().count()) as u32;
        assert!(state.winamp.announcement(start + pass / 2).is_some());
        assert!(
            state
                .winamp
                .announcement(start + pass + std::time::Duration::from_millis(1))
                .is_none()
        );
        state.backend.shutdown();
    }
}
