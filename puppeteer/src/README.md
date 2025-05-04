# kdr-puppeteer

`kdr-puppeteer` allows a websocket server to control a web instance.

## Specs

To make the specs easier to implement, messages from server to client can be in JSON text format or MsgPack binary format. However, messages from client to server must be text. When sending text, the message type must be text. When sending binary data, message type must be binary.

In the future, there might be changes to specs.

| Message               | Text message                                                                                              | From   | Response   | Notes                                                                                                     |
|-----------------------|-----------------------------------------------------------------------------------------------------------|--------|------------|-----------------------------------------------------------------------------------------------------------|
| `request-player-list` | Literal                                                                                                   | Client | PlayerList | Fetching player list to change between players                                                            |
| `change-player`       | Literal                                                                                                   | Client | None       | Requesting spectating a different player. The server will change PuppetFrame accordingly                  |
| PuppetFrame           | `{"PuppetFrame":{"vieworg":[0.0,0.0,0.0],"viewangles":[0.0,0.0,0.0],"server_time":0.0,"timer_time":0.0}}` | Server | None       | Sending client view info                                                                                  |
| ServerTime            | `{"ServerTime":0.0}`                                                                                      | Server | None       | Syncing the client with server time. This is for buffered playback. Buffered playback is not implemented. |
| MapChange             | `{"MapChange":{"game_mod":"cstrike","map_name":"de_dust2"}}`                                              | Server | None       | Changing map                                                                                              |
| PlayerList            | `{"PlayerList":["this","is","it"]}`                                                                       | Server | None       | List of players to spectate                                                                               |
