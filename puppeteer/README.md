# kdr-puppeteer

`kdr-puppeteer` allows a websocket server to control a web instance.

## Specs

To make the specs easier to implement, messages from server to client can be in JSON text format or MsgPack binary format. For MsgPack, the struct are represented by C struct format.

Enum:

```rust
pub enum PuppetEvent {
    PuppetFrame(PuppetFrame),
    MapChange { game_mod: String, map_name: String },
    Version(u32),
}
```

This should be the order when implementing server.

0. Client loads, creates connection with resource provider, initializes render context and GUI components
1. Client starts establishes websocket connection with websocket server.
2. Server sends MapChange and Version and client receives it
3. Server sends PuppetFrame and client receives it
4. Server might send PlayerList and client might receive it as a result.
5. Repeating step 2-5.

### PuppetFrame

Sending client view infos. `server_time` is for syncing with rewinding (unimplemented).

In this example, there are 3 view info for 3 players.

Struct:

```rust
pub struct PuppetFrame {
    server_time: f32,
    /// A list of viewinfos for every spectate-able entities in the server
    frame: Vec<ViewInfo>,
}

pub struct ViewInfo {
    /// Information related to the player
    player: PlayerInfo,
    /// View origin
    vieworg: [f32; 3],
    /// View angles [PITCH, YAW, ROLL]
    viewangles: [f32; 3],
    /// Timer time
    timer_time: f32,
}

struct PlayerInfo {
    name: String,
    steam_id: String,
}
```

JSON:

```json
{
  "PuppetFrame": {
    "server_time": 0,
    "frame": [
      {
        "player": {
          "name": "arte",
          "steam_id": "1234"
        },
        "vieworg": [
          0,
          0,
          0
        ],
        "viewangles": [
          0,
          0,
          0
        ],
        "timer_time": 0
      },
      {
        "player": {
          "name": "arte",
          "steam_id": "1234"
        },
        "vieworg": [
          0,
          0,
          0
        ],
        "viewangles": [
          0,
          0,
          0
        ],
        "timer_time": 0
      },
      {
        "player": {
          "name": "arte",
          "steam_id": "1234"
        },
        "vieworg": [
          0,
          0,
          0
        ],
        "viewangles": [
          0,
          0,
          0
        ],
        "timer_time": 0
      }
    ]
  }
}
```

MsgPack:

```text
[129, 171, 80, 117, 112, 112, 101, 116, 70, 114, 97, 109, 101, 146, 202, 0, 0, 0, 0, 147, 148, 146, 164, 97, 114, 116, 101, 164, 49, 50, 51, 52, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 148, 146, 164, 97, 114, 116, 101, 164, 49, 50, 51, 52, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 148, 146, 164, 97, 114, 116, 101, 164, 49, 50, 51, 52, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 147, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0, 202, 0, 0, 0, 0]
```

### MapChange

Changing map

JSON:

```json
{"MapChange":{"game_mod":"cstrike","map_name":"de_dust2"}}
```

MsgPack:

```text
[129, 169, 77, 97, 112, 67, 104, 97, 110, 103, 101, 146, 167, 99, 115, 116, 114, 105, 107, 101, 168, 100, 101, 95, 100, 117, 115, 116, 50]
```

### Version

To ensure correct client decoding, the version must be incremented whenever the structure or content of any message defined in this specification is modified. The default version is 0. Adding new message types does not require a version change.

JSON:

```json
{"Version":0}
```

MsgPack:

```text
[129, 167, 86, 101, 114, 115, 105, 111, 110, 0]
```
