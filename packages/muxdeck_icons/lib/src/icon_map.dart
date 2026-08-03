/// The curated icon set a deck button may use.
library;

import 'package:flutter/material.dart';

/// Name → icon, for every icon a profile may reference.
///
/// **This must be a `const` map referencing each `IconData` directly.** Flutter tree-shakes
/// icons: it keeps only the glyphs it can see referenced at compile time. A runtime lookup like
/// `IconData(codePoint, fontFamily: 'MaterialIcons')` compiles and works perfectly in debug,
/// then renders **blank squares in release builds** — a bug that only appears after shipping.
/// Because this constant names each icon, the tree-shaker keeps exactly these and nothing else.
///
/// Do not work around that by building with `--no-tree-shake-icons`; it bloats every build to
/// fix one lookup. See `docs/CLIENT.md` §5.
///
/// The deck renders from this map and the desktop icon picker offers from it, so the two cannot
/// disagree about what a name means or which names exist.
const Map<String, IconData> deckIcons = <String, IconData>{
  // Editing
  'content_copy': Icons.content_copy,
  'content_paste': Icons.content_paste,
  'content_cut': Icons.content_cut,
  'undo': Icons.undo,
  'redo': Icons.redo,
  'select_all': Icons.select_all,
  'save': Icons.save,
  'save_alt': Icons.save_alt,
  'search': Icons.search,
  'find_replace': Icons.find_replace,
  'edit': Icons.edit,
  'delete': Icons.delete,
  'add': Icons.add,
  'remove': Icons.remove,
  'check': Icons.check,
  'close': Icons.close,
  'clear': Icons.clear,
  'refresh': Icons.refresh,
  'print': Icons.print,
  'file_copy': Icons.file_copy,
  'folder': Icons.folder,
  'folder_open': Icons.folder_open,
  'description': Icons.description,
  'note_add': Icons.note_add,
  'attach_file': Icons.attach_file,
  'link': Icons.link,
  'bookmark': Icons.bookmark,
  'star': Icons.star,
  'flag': Icons.flag,
  'label': Icons.label,

  // Windows and navigation
  'desktop_windows': Icons.desktop_windows,
  'swap_horiz': Icons.swap_horiz,
  'swap_vert': Icons.swap_vert,
  'grid_view': Icons.grid_view,
  'view_column': Icons.view_column,
  'fullscreen': Icons.fullscreen,
  'fullscreen_exit': Icons.fullscreen_exit,
  'open_in_new': Icons.open_in_new,
  'arrow_back': Icons.arrow_back,
  'arrow_forward': Icons.arrow_forward,
  'arrow_upward': Icons.arrow_upward,
  'arrow_downward': Icons.arrow_downward,
  'first_page': Icons.first_page,
  'last_page': Icons.last_page,
  'home': Icons.home,
  'menu': Icons.menu,
  'more_horiz': Icons.more_horiz,
  'keyboard_return': Icons.keyboard_return,
  'keyboard_tab': Icons.keyboard_tab,
  'keyboard': Icons.keyboard,
  'tab': Icons.tab,
  'window': Icons.window,
  'minimize': Icons.minimize,
  'crop_square': Icons.crop_square,

  // Media
  'play_arrow': Icons.play_arrow,
  'pause': Icons.pause,
  'play_pause': Icons.play_circle,
  'stop': Icons.stop,
  'skip_next': Icons.skip_next,
  'skip_previous': Icons.skip_previous,
  'fast_forward': Icons.fast_forward,
  'fast_rewind': Icons.fast_rewind,
  'volume_up': Icons.volume_up,
  'volume_down': Icons.volume_down,
  'volume_off': Icons.volume_off,
  'volume_mute': Icons.volume_mute,
  'mic': Icons.mic,
  'mic_off': Icons.mic_off,
  'headphones': Icons.headphones,
  'music_note': Icons.music_note,
  'queue_music': Icons.queue_music,
  'repeat': Icons.repeat,
  'shuffle': Icons.shuffle,
  'speaker': Icons.speaker,

  // Capture and streaming
  'photo_camera': Icons.photo_camera,
  'videocam': Icons.videocam,
  'videocam_off': Icons.videocam_off,
  'screenshot_monitor': Icons.screenshot_monitor,
  'screen_share': Icons.screen_share,
  'stop_screen_share': Icons.stop_screen_share,
  'radio_button_checked': Icons.radio_button_checked,
  'live_tv': Icons.live_tv,
  'movie': Icons.movie,
  'image': Icons.image,
  'crop': Icons.crop,
  'palette': Icons.palette,
  'brush': Icons.brush,
  'layers': Icons.layers,
  'visibility': Icons.visibility,
  'visibility_off': Icons.visibility_off,

  // System
  'lock': Icons.lock,
  'lock_open': Icons.lock_open,
  'power_settings_new': Icons.power_settings_new,
  'settings': Icons.settings,
  'tune': Icons.tune,
  'brightness_high': Icons.brightness_high,
  'brightness_low': Icons.brightness_low,
  'dark_mode': Icons.dark_mode,
  'light_mode': Icons.light_mode,
  'wifi': Icons.wifi,
  'wifi_off': Icons.wifi_off,
  'bluetooth': Icons.bluetooth,
  'battery_full': Icons.battery_full,
  'memory': Icons.memory,
  'storage': Icons.storage,
  'computer': Icons.computer,
  'phone_android': Icons.phone_android,
  'usb': Icons.usb,
  'cable': Icons.cable,
  'monitor': Icons.monitor,

  // Communication
  'mail': Icons.mail,
  'send': Icons.send,
  'chat': Icons.chat,
  'call': Icons.call,
  'call_end': Icons.call_end,
  'notifications': Icons.notifications,
  'notifications_off': Icons.notifications_off,
  'person': Icons.person,
  'group': Icons.group,
  'share': Icons.share,

  // Development
  'code': Icons.code,
  'terminal': Icons.terminal,
  'bug_report': Icons.bug_report,
  'build': Icons.build,
  'play_circle_outline': Icons.play_circle_outline,
  'stop_circle': Icons.stop_circle,
  'sync': Icons.sync,
  'cloud_upload': Icons.cloud_upload,
  'cloud_download': Icons.cloud_download,
  'merge': Icons.merge,
  'commit': Icons.commit,
  'account_tree': Icons.account_tree,
  'data_object': Icons.data_object,
  'api': Icons.api,
  'rocket_launch': Icons.rocket_launch,

  // Generic
  'circle': Icons.circle,
  'square': Icons.square,
  'bolt': Icons.bolt,
  'favorite': Icons.favorite,
  'thumb_up': Icons.thumb_up,
  'thumb_down': Icons.thumb_down,
  'warning': Icons.warning,
  'info': Icons.info,
  'help': Icons.help,
  'timer': Icons.timer,
  'schedule': Icons.schedule,
  'alarm': Icons.alarm,
  'calendar_today': Icons.calendar_today,
  'shopping_cart': Icons.shopping_cart,
  'attach_money': Icons.attach_money,
  'language': Icons.language,
  'map': Icons.map,
  'place': Icons.place,
  'accessibility': Icons.accessibility,
  'auto_awesome': Icons.auto_awesome,
};

/// What an unrecognised icon name renders as.
///
/// A filled dot rather than nothing: a button that draws blank looks broken, whereas a dot
/// reads as "this works, its icon just did not resolve". `docs/PROTOCOL.md` §6.
const IconData fallbackIcon = Icons.circle;

/// The icon for a profile's `icon` string, falling back rather than failing.
IconData iconFor(String name) => deckIcons[name] ?? fallbackIcon;

/// Every name a profile may use, sorted — the icon picker's source of truth.
///
/// Because the picker offers only these, it can never propose a name the deck would draw as a
/// fallback dot.
List<String> get iconNames => deckIcons.keys.toList()..sort();
