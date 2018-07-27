# Meowww !

This is an extremely simple in-browser chat system.

The server is self contained after compilation in release mode.
Chat content is not logged and not stored on disk, so it is destroyed when the server is closed.
Chat supports multiple independent chat rooms, with no discovery.
Chat rooms are created on the fly, when first used.

There is no connection system, thus nicknames are not unique and not reserved for one user.
Nickname is only defined for a specific message, and can be changed anytime.
To simplify use, the nickname is stored in a cookie to be remembered accross sessions.

## Requirements
Server: Rust.

Client: Web browser with Websockets.

## Design
The server uses rouille, which has synchronous IO.

WebSockets are used only to push new message notifications from the server to clients.
They are technically duplex mode, but the rouille API is too synchronous to use both directions.
New messages are sent to the server using separate AJAX requests.

Each chat room is protected by a single lock, which prevents scaling to many users in a chat room.
However many chat rooms can be supported without too much slowdown.
The list of chat rooms is protected by a RwLock, so creation / destruction of chat rooms is serialized.

## TODO
* Prune unused chat rooms (timer).
* Improve websocket handling with an asynchronous framework (not soon).
