# Meowww !

This is an extremely simple in-browser chat system.

The server is self contained after compilation in release mode.
Chat content is not logged and not stored on disk, so it is destroyed when the server is closed.
Chat supports multiple independent chat rooms, with no discovery.

## Requirements
Server: Rust.

Client: Web browser with Websockets.

## Design
The server uses rouille, which has synchronous IO.
All requests must go through a lock (bad), but this is simpler.

WebSockets are used only to push new message notifications from the server to clients.
They are technically duplex mode, but the rouille API is too synchronous to use both directions.
New messages are sent to the server using separate AJAX requests.

## TODO
* Change icon in case of notification ?
* Store nickname in Cookies ?
* Improve websocket handling & cleanup, by using an asynchronous framework.
* Prune unused chat rooms (timer).
