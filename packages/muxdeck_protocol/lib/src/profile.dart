/// Deck layouts. `docs/PROTOCOL.md` §4.5 and §6.
library;

import 'envelope.dart';

/// One deck layout: a grid of pages of buttons.
class Profile {
  const Profile({
    required this.id,
    required this.name,
    required this.grid,
    required this.pages,
  });

  factory Profile.fromJson(Map<String, dynamic> json) => Profile(
    id: json['id'] as String,
    name: json['name'] as String,
    grid: Grid.fromJson(json['grid'] as Map<String, dynamic>),
    pages: (json['pages'] as List<dynamic>)
        .map((p) => Page.fromJson(p as Map<String, dynamic>))
        .toList(),
  );

  final String id;
  final String name;
  final Grid grid;
  final List<Page> pages;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'grid': grid.toJson(),
    'pages': pages.map((p) => p.toJson()).toList(),
  };
}

class Grid {
  const Grid({required this.cols, required this.rows});

  factory Grid.fromJson(Map<String, dynamic> json) =>
      Grid(cols: json['cols'] as int, rows: json['rows'] as int);

  final int cols;
  final int rows;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'cols': cols,
    'rows': rows,
  };
}

class Page {
  const Page({required this.id, required this.name, required this.buttons});

  factory Page.fromJson(Map<String, dynamic> json) => Page(
    id: json['id'] as String,
    name: json['name'] as String,
    buttons: (json['buttons'] as List<dynamic>)
        .map((b) => Button.fromJson(b as Map<String, dynamic>))
        .toList(),
  );

  final String id;
  final String name;

  /// Buttons are sparse: a grid cell with no button is empty.
  final List<Button> buttons;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'buttons': buttons.map((b) => b.toJson()).toList(),
  };
}

class Button {
  const Button({
    required this.id,
    required this.pos,
    required this.label,
    required this.icon,
    required this.color,
    required this.haptic,
    this.onTap,
    this.onLongPress,
  });

  factory Button.fromJson(Map<String, dynamic> json) => Button(
    id: json['id'] as String,
    pos: Position.fromJson(json['pos'] as Map<String, dynamic>),
    label: json['label'] as String,
    icon: json['icon'] as String,
    color: json['color'] as String,
    haptic: Haptic.fromWire(json['haptic'] as String),
    onTap: json['on_tap'] == null
        ? null
        : ButtonAction.fromJson(json['on_tap'] as Map<String, dynamic>),
    onLongPress: json['on_long_press'] == null
        ? null
        : ButtonAction.fromJson(json['on_long_press'] as Map<String, dynamic>),
  );

  final String id;
  final Position pos;
  final String label;

  /// A name from the curated icon map. Unknown names fall back to a filled dot rather than
  /// rendering blank — Flutter tree-shakes icons, so a runtime string lookup into the full
  /// Material set silently renders nothing in release builds.
  final String icon;

  /// `#RRGGBB`.
  final String color;
  final Haptic haptic;
  final ButtonAction? onTap;
  final ButtonAction? onLongPress;

  /// Both actions are emitted even when null, because the wire carries an explicit `null`
  /// and dropping the key would change the message.
  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'pos': pos.toJson(),
    'label': label,
    'icon': icon,
    'color': color,
    'haptic': haptic.wire,
    'on_tap': onTap?.toJson(),
    'on_long_press': onLongPress?.toJson(),
  };
}

class Position {
  const Position({required this.col, required this.row});

  factory Position.fromJson(Map<String, dynamic> json) =>
      Position(col: json['col'] as int, row: json['row'] as int);

  final int col;
  final int row;

  Map<String, dynamic> toJson() => <String, dynamic>{'col': col, 'row': row};
}

enum Haptic {
  none('none'),
  light('light'),
  medium('medium'),
  heavy('heavy');

  const Haptic(this.wire);

  final String wire;

  static Haptic fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () =>
        throw ProtocolException(ErrorCode.badRequest, 'unknown haptic "$wire"'),
  );
}

/// An embedded `{ op, d }` pair.
///
/// [d] stays a raw map on purpose: the op it belongs to is only known at dispatch time, and
/// the engine re-checks both the op's permissibility and the payload's shape when the
/// button is pressed rather than trusting what was stored.
class ButtonAction {
  const ButtonAction(this.op, this.d);

  factory ButtonAction.fromJson(Map<String, dynamic> json) => ButtonAction(
    Op.parse(json['op'] as String),
    json['d'] as Map<String, dynamic>,
  );

  final Op op;
  final Map<String, dynamic> d;

  Map<String, dynamic> toJson() => <String, dynamic>{'op': op.wire, 'd': d};
}

/// The payload of `profile.get`'s response, `profile.set`'s request and the
/// `profile.changed` event — one wrapper for all three, which is why the Profile is wrapped
/// rather than being the payload itself.
class ProfileWrapper implements Payload {
  const ProfileWrapper(this.profile);

  factory ProfileWrapper.fromJson(Map<String, dynamic> json) =>
      ProfileWrapper(Profile.fromJson(json['profile'] as Map<String, dynamic>));

  final Profile profile;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'profile': profile.toJson(),
  };
}

class ProfileGetRequest implements Payload {
  const ProfileGetRequest(this.profileId);

  factory ProfileGetRequest.fromJson(Map<String, dynamic> json) =>
      ProfileGetRequest(json['profile_id'] as String);

  final String profileId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'profile_id': profileId};
}

class ProfileListResponse implements Payload {
  const ProfileListResponse(this.profiles);

  factory ProfileListResponse.fromJson(Map<String, dynamic> json) =>
      ProfileListResponse(
        (json['profiles'] as List<dynamic>)
            .map((p) => ProfileSummary.fromJson(p as Map<String, dynamic>))
            .toList(),
      );

  final List<ProfileSummary> profiles;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'profiles': profiles.map((p) => p.toJson()).toList(),
  };
}

class ProfileSummary {
  const ProfileSummary({
    required this.id,
    required this.name,
    required this.active,
  });

  factory ProfileSummary.fromJson(Map<String, dynamic> json) => ProfileSummary(
    id: json['id'] as String,
    name: json['name'] as String,
    active: json['active'] as bool,
  );

  final String id;
  final String name;
  final bool active;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'active': active,
  };
}

class ProfileActivateRequest implements Payload {
  const ProfileActivateRequest(this.profileId);

  factory ProfileActivateRequest.fromJson(Map<String, dynamic> json) =>
      ProfileActivateRequest(json['profile_id'] as String);

  final String profileId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'profile_id': profileId};
}

class ProfileDeleteRequest implements Payload {
  const ProfileDeleteRequest(this.profileId);

  factory ProfileDeleteRequest.fromJson(Map<String, dynamic> json) =>
      ProfileDeleteRequest(json['profile_id'] as String);

  final String profileId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'profile_id': profileId};
}
