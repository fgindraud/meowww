#[macro_use]
extern crate rouille;
#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate rust_embed;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

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
        .arg(
            clap::Arg::with_name("history_size")
                .short("l")
                .long("history-size")
                .takes_value(true)
                .value_name("size")
                .default_value("1000")
                .help("Maximum size of room history"),
        )
        .get_matches();

    let history_size = matches
        .value_of("history_size")
        .unwrap()
        .parse()
        .expect("history_size: usize");
    start_server(matches.value_of("addr").unwrap(), history_size)
}

fn start_server(addr: &str, history_size: usize) {
    eprintln!("Meowww starting on {}", addr);
    // use Mutex instead of Rwlock, because Room is not Sync (due to Client / Websocket).
    let rooms = Mutex::new(HashMap::<String, Room>::new());
    rouille::start_server(addr, move |request| {
        router!(request,
            (GET) ["/"] => { home_page(request) },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room}", room: String] => {
                let lock = rooms.lock().unwrap();
                room_page(&room, lock.get(&room).map(|r : &Room| r.history()))
            },
            (POST) ["/{room}", room: String] => {
                let mut lock = rooms.lock().unwrap();
                post_message(request, lock.entry(room).or_insert_with(|| Room::new(history_size)))
            },
            (GET) ["/{room}/notify", room: String] => {
                let mut lock = rooms.lock().unwrap();
                create_notify_websocket(request, lock.entry(room).or_insert_with(|| Room::new(history_size)))
            },
            _ => { Response::empty_404() }
        )
    })
}

/******************************************************************************
 * Start the server.
 */
#[derive(Clone, Serialize, Deserialize)]
struct Message {
    nickname: String,
    content: String,
}

struct Room {
    history_size: usize,
    history: VecDeque<Message>,
    clients: Vec<Client>,
}

impl Room {
    fn new(history_size: usize) -> Self {
        Room {
            history_size: history_size,
            history: VecDeque::new(),
            clients: Vec::new(),
        }
    }
    fn add_message(&mut self, message: Message) {
        // Do not propagate degenerate messages
        if !message.nickname.trim().is_empty() && !message.content.trim().is_empty() {
            notify_clients(&mut self.clients, &message);
            self.history.push_back(message);
            while self.history.len() > self.history_size {
                self.history.pop_front();
            }
        }
    }
    fn history(&self) -> &VecDeque<Message> {
        &self.history
    }
    fn add_client(&mut self, socket: mpsc::Receiver<Websocket>) {
        self.clients.push(Client::Pending(socket))
    }
}

/* Connection to client for notification.
 * Due to rouille websocket API (synchronous), we only use these sockets to push notifications.
 * They are only destroyed when trying to send data, during notifications.
 * TODO periodic cleanup ?
 */
enum Client {
    Pending(mpsc::Receiver<Websocket>),
    Connected(Websocket),
}

/* Notification:
 * Send the message to each connected client.
 * Drop any client with an error.
 * If client was not connected yet (Pending), finish connection.
 */
fn notify_clients(clients: &mut Vec<Client>, message: &Message) {
    let json = serde_json::to_string(message).unwrap();

    let mut notified_clients = Vec::new();
    for client in clients.drain(..) {
        let mut socket = match client {
            Client::Pending(receiver) => receiver.recv().unwrap(),
            Client::Connected(socket) => socket,
        };
        if let Ok(_) = socket.send_text(&json) {
            // Drop if failed to send, keep on success
            notified_clients.push(Client::Connected(socket))
        }
    }
    *clients = notified_clients;
}

/******************************************************************************
 * Webpage templates.
 */
fn home_page(request: &Request) -> Response {
    let server = request.header("Host").unwrap_or("<server>");
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                link(rel="icon", type="image/vnd.microsoft.icon", href="/static/meowww.ico");
                meta(name="viewport", content="width=device-width, initial-scale=1.0");
                title : "Meowww !";
            }
            body {
                p : format!("Go to http://{}/<room_name> to access a chat room.", server);
                p : "Conversations are not stored on disk, so save the page if you want to keep them !";
            }
        }
    };
    Response::html(template.into_string().unwrap())
}

fn room_page(room: &str, history: Option<&VecDeque<Message>>) -> Response {
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                link(rel="icon", type="image/vnd.microsoft.icon", href="/static/meowww.ico");
                link(rel="stylesheet", type="text/css", href="/static/style.css");
                meta(name="viewport", content="width=device-width, initial-scale=1.0");
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
    Response::text("Message sent. Please enable javascript.")
}

fn create_notify_websocket(request: &Request, room: &mut Room) -> Response {
    use rouille::websocket;
    let (response, websocket_receiver) = try_or_400!(websocket::start(request, Some("meowww")));
    /* rouille::websocket:
     * start returns a response that must be sent before access to the websocket.
     * The current strategy is to store the mpsc::Receiver.
     * The socket is received later during a notification.
     */
    room.add_client(websocket_receiver);
    response
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
            path if path.ends_with(".ico") => "image/vnd.microsoft.icon",
            _ => "application/octet-stream",
        };
        Response::from_data(content_type, asset).with_public_cache(3600)
    } else {
        Response::empty_404()
    }
}
