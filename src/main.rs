#[macro_use]
extern crate rouille;
#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate rust_embed;

use horrorshow::Template;
use rouille::{websocket::Websocket, Request, Response};
use std::collections::{HashMap, VecDeque};
use std::sync::{mpsc, Mutex};

/******************************************************************************
 * Start the server.
 */
fn main() {
    let matches = app_from_crate!()
        .arg(
            clap::Arg::with_name("addr")
                .help("Address on which the server will bind")
                .default_value("localhost:8000"),
        )
        .get_matches();

    start_server(matches.value_of("addr").unwrap())
}

fn start_server(addr: &str) {
    eprintln!("Meowww starting on {}", addr);
    // use Mutex instead of Rwlock, because Room is not Sync (due to Client / Websocket).
    let rooms = Mutex::new(HashMap::<String, Room>::new());
    rouille::start_server(addr, move |request| {
        router!(request,
            (GET) ["/"] => { home_page() },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room}", room: String] => {
                let lock = rooms.lock().unwrap();
                room_page(&room, lock.get(&room).map(|r : &Room| r.history()))
            },
            (POST) ["/{room}", room: String] => {
                let mut lock = rooms.lock().unwrap();
                post_message(request, lock.entry(room.clone()).or_insert_with(|| Room::new(room)))
            },
            (GET) ["/{room}/notify", room: String] => {
                let mut lock = rooms.lock().unwrap();
                create_notify_websocket(request, lock.entry(room.clone()).or_insert_with(|| Room::new(room)))
            },
            _ => { Response::empty_404() }
        )
    })
}

/******************************************************************************
 * Start the server.
 */
#[derive(Clone)]
struct Message {
    nickname: String,
    content: String,
}

// TODO history limit
struct Room {
    name: String,
    history: VecDeque<Message>,
    clients: Vec<Client>,
}

impl Room {
    fn new<S: Into<String>>(name: S) -> Self {
        Room {
            name: name.into(),
            history: VecDeque::new(),
            clients: Vec::new(),
        }
    }
    fn name(&self) -> &String {
        &self.name
    }
    fn add_message(&mut self, message: Message) {
        self.history.push_back(message);
        notify_clients(&mut self.clients)
    }
    fn history(&self) -> &VecDeque<Message> {
        &self.history
    }
    fn add_client(&mut self, socket: mpsc::Receiver<Websocket>) {
        self.clients.push(Client::Pending(socket))
    }
}

/* Notification:
 * Send the message to each connected client.
 * Drop any client with an error.
 * If client was not connected yet (Pending), finish connection.
 * FIXME send actual message
 */
enum Client {
    Pending(mpsc::Receiver<Websocket>),
    Connected(Websocket),
}
fn notify_clients(clients: &mut Vec<Client>) {
    let mut notified_clients = Vec::new();
    for client in clients.drain(..) {
        let mut socket = match client {
            Client::Pending(receiver) => receiver.recv().unwrap(),
            Client::Connected(socket) => socket,
        };
        if let Ok(_) = socket.send_text("Blah") {
            notified_clients.push(Client::Connected(socket))
        }
    }
    *clients = notified_clients;
}

/******************************************************************************
 * Webpage templates.
 */
fn room_page(room: &str, history: Option<&VecDeque<Message>>) -> Response {
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                link(rel="stylesheet", type="text/css", href="/static/style.css");
                title : format!("Meowww - {}", room);
            }
            body {
                main {
                    table {
                        @ if let Some(messages) = history {
                            @ for m in messages.iter() {
                                tr {
                                    td : &m.nickname;
                                    td : &m.content;
                                }
                            }
                        }
                    }
                }
                footer {
                    form(autocomplete="off", method="post") {
                        input(type="text", name="nickname", placeholder="Nickname");
                        input(type="text", name="content", placeholder="Message content", autofocus);
                        input(type="submit", value="Send");
                    }
                }
                script(src="/static/jquery.js") {}
                script(src="/static/client.js") {}
            }
        }
    };
    Response::html(template.into_string().unwrap())
}

fn post_message(request: &Request, room: &mut Room) -> Response {
    let form_data = try_or_400!(post_input!(request, { nickname: String, content: String }));
    let message = Message {
        nickname: form_data.nickname,
        content: form_data.content,
    };
    room.add_message(message);
    Response::redirect_303(format!("/{}", room.name()))
}

fn create_notify_websocket(request: &Request, room: &mut Room) -> Response {
    use rouille::websocket;
    let (response, websocket_receiver) = try_or_400!(websocket::start(request, Some("meowww")));
    room.add_client(websocket_receiver);
    response
}

fn home_page() -> Response {
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                title : "Meowww";
            }
            body {
                p : "Go to http://<server>/<chat_room_name> to access a chat room.";
            }
        }
    };
    Response::html(template.into_string().unwrap())
}

/* External files.
 * Static files are much easier to edit as standalone.
 * Use rust_embed to embed them in the binary (on release mode only).
 */
#[derive(RustEmbed)]
#[folder = "www/"]
struct Asset;

fn send_asset(path: &str) -> Response {
    if let Some(asset) = Asset::get(path) {
        let content_type = match path {
            path if path.ends_with(".css") => "text/css",
            path if path.ends_with(".js") => "application/javascript",
            _ => "application/octet-stream",
        };
        Response::from_data(content_type, asset) // TODO Add .with_public_cache(3600)
    } else {
        Response::empty_404()
    }
}
