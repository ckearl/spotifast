---
title: The Winamp Mini Player
description: Use classic Winamp 2 skins with an analyser, equalizer, and playlist.
nav_order: 4
---

Open the mini player with Ctrl+M (Cmd+Shift+M on macOS), the shrink button, or
**Switch to it** in Settings. It supports classic Winamp 2 `.wsz` skins. Find
skins at the [Winamp Skin Museum](https://skins.webamp.org).

Only one player window is open at a time. Click the skin logo or Eject, or use
the shortcut again, to return to the main window.

![The mini player wearing the built-in skin](/assets/images/winamp.png)

## Skins and window size

Drop a `.wsz` file on either window to install and use it. Settings lists the
installed skins and can open the skins folder.

Choose **Random**, first in that list, to get a different skin each time you
switch to the mini player, never the same one twice in a row. Settings says
which skin it picked, and choosing a skin yourself turns Random off.

You can also use an unpacked skin folder. Spotifast finds skin files inside
its subfolders, up to eight folders deep, so you do not need to move them all
into one folder.

Right-click the title bar, or click **O**, to choose a size from 1x to 4x.
Each size keeps the classic pixels sharp. The same menu can keep the player
above other windows. **D** toggles double size and **A** toggles always-on-top.
Spotifast remembers the window position where your desktop allows it.

Skins can have transparent areas and shapes other than rectangles. Modern
Winamp 3 and 5 skin formats are not supported; choose classic Winamp 2 skins.

## Main controls

Most controls match Winamp. As in Winamp, Play on a song that is already
playing starts it again from the beginning; Pause pauses. These work
differently:

- **Stop** pauses and rewinds.
- **I** opens the playing album in the main window.
- Repeat is either on or off.
- The X button and both logos return to the main window.

Click the time to switch between elapsed and remaining time. The balance and
MONO/STEREO controls affect playback on this computer. Quit from the
right-click menu or with Ctrl+Q.

The shade button, or a double-click on the title bar, rolls the player up. The
playlist and equalizer have their own shade buttons.

Switching to the main window and back keeps the shade modes and restores the
mini player's own position. It does not inherit the main window's fullscreen
or maximized state. On Linux desktops using Wayland, the desktop chooses where
windows appear and may ignore the saved position.

The left display shows bars that react to the sound (a spectrum analyser).
Click it to switch to a moving sound wave (an oscilloscope), then off.
You can also use the **V** menu. Changes to the equalizer affect the picture;
turning down the volume does not, even at zero. The display reacts only to
music playing on this computer. **VIS** below the player has a bigger graph.

## Playlist

**PL** opens the playlist below the player. It shows the playing song followed
by the queue. Double-click a song to play it, Ctrl-click to select several, and
drag the lower-right corner to resize the window. Use X or **PL** to close it.

Right-click a song to play it, queue it, add it to a playlist, like it, go to
its artist or album, start its radio, or copy its link. Its artist, album and
radio open in **FIND**, not the main window.

- **ADD → Search** opens **FIND**. ADD also opens Liked Songs.
- **SEL** selects rows.
- **MISC** opens song, artist, and album pages.
- **LIST OPTS** starts one of your playlists or saves the queue as a new one.
- **REM → Remove selected** takes the selected songs you queued out of the
  queue, and **REM → Remove all** clears every song you queued. Both work when
  this computer is playing. Songs from the playing album or playlist stay.

Notices from the main window scroll through the mini player's text display.

## The four toggles

**FIND**, **LYR**, **VIS** and **DEV** are drawn under the transport buttons.
FIND, LYR and VIS each open a window below the player, and are also in the
**O** menu. Like the playlist, each can be rolled up with its shade button,
closed with X, and stretched by dragging its lower-right corner. **DEV** opens
the device menu, described under Devices below.

## FIND

**FIND** searches Spotify and browses your library without leaving the mini
player. Open it with the FIND button, **ADD → Search**, **/**, or Ctrl+F
(Cmd+F on macOS).

With the box empty, the row under it reads **RECENT**, **SONGS**, **ARTISTS**,
**ALBUMS** and **LISTS**. RECENT is what you played lately. SONGS is your Liked
Songs, ARTISTS the artists you follow, ALBUMS your saved albums and LISTS your
playlists. Start typing and the list becomes a search of the kind that is
selected; RECENT turns into SONGS. Click another kind to search for that
instead. A search shows up to 15 results. It does not go into the Recent
searches of the main window.

- Double-click a song, or press Enter, to play it.
- Artists, albums, playlists and radio open inside FIND. An artist's page has
  its own **SONGS** and **ALBUMS** row.
- **BACK**, or Backspace in an empty box, goes back a page. Hold the key down
  and it still goes back only once.
- The bar at the bottom reads **PLAY**, **+QUEUE**, **+LIST**, **RADIO** and
  **BACK**. +LIST adds the picked song to one of your playlists. The playlist
  picker here has no New playlist; make those in the main window.
- Right-click a row in FIND or in the playlist to see what you can do with it.
  **Remove from Your Library** is never offered for a playlist you own. Add to
  queue on a playlist loads the whole playlist first, then queues it.

**Esc** steps out one level at a time. In the box it leaves the box and keeps
your results, so you can use the keys on them. Outside the box it clears the
text. With the box empty it closes FIND.

## LYRICS

**LYR** shows the lyrics of the playing song. When the lyrics are synced, the
current line is highlighted and the window follows it. Click a line to jump the
song to that line. Scrolling pauses the following; it starts again when you
click a synced line, which also jumps there, or when the track changes. If the
lyrics could not be loaded, the window says so in a line you can click to try
again.

## VIS

**VIS** is a wide bar graph in the skin's own visualizer colours, bigger than
the little display at the top left. It reacts only to music playing on this
computer, keeps moving at zero volume, and follows the equalizer. When the music
is playing on another device, it says so instead.

## Devices

**DEV** lists this computer and your Spotify Connect devices, with a check mark
on the one that is playing. Click one to move the music there. Devices that
Spotify marks as restricted are greyed out, but you can still choose them. The
title display says "Playing on …" while the music is on another device.

## Keys

The mini player has its own keys, alongside the usual ones such as Space. They
do nothing while you type in the FIND box. Some take two keys, one after the
other: the first shows in the title display until the second arrives, and is
forgotten after a second. A key that is not a letter, such as Space, cancels
it and then does its normal job. The same list is in the **?** shortcuts
dialog.

| Key | Does |
| --- | --- |
| Ctrl+F or / (Cmd+F or / on macOS) | Search |
| E | Equalizer |
| Q | Playlist |
| L | Lyrics |
| V | Spectrum analyser |
| Y | Copy link |
| G A | Go to artist |
| G B | Go to album |
| G R | Radio |
| G H | Recently played |
| G Y | Library |
| G D | Connect to a device |
| G P | Go to the playing song |
| A Q | Add to queue |
| A N | Play next |
| A P | Add to playlist |
| A L | Like, save or follow |
| D L | Unlike, unsave or unfollow |
| D P | Remove from this playlist |
| D Q | Remove from queue |

The two-key commands act on the row picked in FIND or the playlist, or on the
playing song when nothing is picked.

## Equalizer

**EQ** opens the ten-band equalizer. It affects playback on this computer, not
other Spotify Connect devices. The preamp ranges from -12 to 12 dB. **AUTO**
resets all bands. The same controls and presets are in Settings.

To keep your own curve, such as a headphone correction, type a name under the
presets in **Settings → Equalizer** and choose **Save preset**. It stores the
preamp and the ten bands, and appears after the built-in presets in Settings
and in the mini player's **PRESETS** menu. Saving under a name you already
used replaces that preset. While a saved preset is in use, **Delete** beside
**Save preset** removes it; the sound stays as it is.
