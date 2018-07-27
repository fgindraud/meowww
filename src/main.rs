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
use rouille::url::percent_encoding::{percent_decode, utf8_percent_encode, USERINFO_ENCODE_SET};
use rouille::{websocket::Websocket, Request, Response};
use std::collections::{HashMap, VecDeque};
use std::sync::{mpsc, Mutex, RwLock};

macro_rules! debug {
    ($($arg:tt)*) => { 
        if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        }
    };
}

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
        .arg(
            clap::Arg::with_name("nb_threads")
                .short("n")
                .long("nb-threads")
                .takes_value(true)
                .default_value("2")
                .help("Size of the threadpool"),
        )
        .get_matches();

    let history_size = matches
        .value_of("history_size")
        .unwrap()
        .parse()
        .expect("history_size: usize");
    let nb_threads = matches
        .value_of("nb_threads")
        .unwrap()
        .parse()
        .expect("nb_threads: usize");
    start_server(matches.value_of("addr").unwrap(), history_size, nb_threads)
}

fn start_server(addr: &str, history_size: usize, nb_threads: usize) {
    eprintln!("Meowww starting on {}", addr);
    let rooms = Rooms::new(history_size);
    rouille::start_server_with_pool(addr, Some(nb_threads), move |request| {
        router!(request,
            (GET) ["/"] => { home_page(request) },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room_name}", room_name: String] => {
                rooms.access(&room_name, |opt_room| room_page(request, &room_name, opt_room.map(|r| r.history())))
            },
            (POST) ["/{room_name}", room_name: String] => {
                rooms.modify(&room_name, |room| post_message(request, room))
            },
            (GET) ["/{room_name}/notify", room_name: String] => {
                rooms.modify(&room_name, |room| create_notify_websocket(request, room))
            },
            _ => { Response::empty_404() }
        )
    })
}

/* Room set synchronisation:
 * The set of rooms is infrequently modified, so the table itself is protected by a RwLock.
 * Each Room is then protected by a mutex.
 * RwLock cannot be used because inner Client Websockets are not Sync.
 * And Rooms are often modified, so a RwLock is not as interesting.
 * Lock acquisition order is always table_rwlock then room_mutex.
 */
struct Rooms {
    table: RwLock<HashMap<String, Mutex<Room>>>,
    history_size: usize,
}

impl Rooms {
    fn new(history_size: usize) -> Self {
        Rooms {
            table: RwLock::new(HashMap::new()),
            history_size: history_size,
        }
    }
    // Access a room in readonly and execute function
    fn access<F, R>(&self, room_name: &str, f: F) -> R
    where
        F: FnOnce(Option<&Room>) -> R,
    {
        let table_lock = self.table.read().unwrap();
        match table_lock.get(room_name) {
            Some(room) => {
                let room_lock = room.lock().unwrap();
                f(Some(&room_lock))
            }
            None => f(None),
        }
    }
    /* Access a room in write mode and execute function.
     * If the room does not exist, create a new room.
     * All modify operation will fill it with something anyway.
     */
    fn modify<F, R>(&self, room_name: &str, f: F) -> R
    where
        F: Fn(&mut Room) -> R,
    {
        if let Some(room) = self.table.read().unwrap().get(room_name) {
            // Fast path using only read lock: room already exists
            return f(&mut room.lock().unwrap());
        }

        use std::collections::hash_map::Entry;
        match self.table.write().unwrap().entry(room_name.to_owned()) {
            Entry::Occupied(mut entry) => {
                // A new room has already been inserted by someone else in the meantime.
                f(&mut entry.get_mut().lock().unwrap())
            }
            Entry::Vacant(entry) => {
                let mut room = Room::new(room_name, self.history_size);
                let result = f(&mut room);
                entry.insert(Mutex::new(room));
                result
            }
        }
    }
}

/******************************************************************************
 * Chat room management.
 */
#[derive(Clone, Serialize, Deserialize)]
struct Message {
    nickname: String,
    content: String,
}

/* Room structure.
 * Keep a fixed size history of messages.
 *
 * Maintain list of connected client websockets for new message notifications.
 * Due to rouille websocket API (synchronous), we only use these sockets to push notifications.
 * Pending clients are not yet completed WebSockets (see create_notify_websocket).
 * Connected clients are complete WebSockets.
 * The list of clients must be cleaned frequently to remove failed sockets.
 * This cannot be done asynchronously due to the limited rouille API.
 */
struct Room {
    name: String,
    history_size: usize,
    history: VecDeque<Message>,
    pending_clients: Vec<mpsc::Receiver<Websocket>>,
    connected_clients: Vec<Websocket>,
}

impl Room {
    fn new<S: Into<String>>(name: S, history_size: usize) -> Self {
        Room {
            name: name.into(),
            history_size: history_size,
            history: VecDeque::new(),
            pending_clients: Vec::new(),
            connected_clients: Vec::new(),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn history(&self) -> &VecDeque<Message> {
        &self.history
    }
    fn nb_clients(&self) -> usize {
        self.pending_clients.len() + self.connected_clients.len()
    }

    fn add_message(&mut self, mut message: Message) {
        // Do not propagate degenerate messages
        if !message.nickname.trim().is_empty() && !message.content.trim().is_empty() {
            // Limit nickname size. Do not use truncate because of char boundary.
            message.nickname = message.nickname.chars().take(30).collect();
            // Add message to history and clients history
            self.notify_clients(&message);
            self.history.push_back(message);
            while self.history.len() > self.history_size {
                self.history.pop_front();
            }
        }
    }
    fn add_client(&mut self, future_socket: mpsc::Receiver<Websocket>) {
        self.pending_clients.push(future_socket);
        debug!("[{}] add Websocket = {}", self.name(), self.nb_clients())
    }

    // Clean the client list.
    fn connect_clients(&mut self) {
        // Connect pending clients
        self.connected_clients
            .extend(self.pending_clients.drain(..).filter_map(|c| c.recv().ok()));
        // Send empty messages to test clients
        self.broadcast_and_clean(&"");
        debug!("[{}] clean Websocket = {}", self.name(), self.nb_clients())
    }

    // Send a message to all clients, and drop failed clients.
    fn notify_clients(&mut self, message: &Message) {
        let json = serde_json::to_string(message).unwrap();
        self.broadcast_and_clean(&json);
        debug!("[{}] send Websocket = {}", self.name(), self.nb_clients())
    }

    fn broadcast_and_clean(&mut self, text: &str) {
        // Send the message to all clients, remove failed and closed ones.
        let non_failed_clients = self.connected_clients
            .drain(..)
            .filter_map(|mut socket| match socket.send_text(text) {
                Ok(_) => Some(socket),
                Err(_) => None,
            })
            .filter(|socket| !socket.is_closed()) // NOTE no effect if no reads :/
            .collect();
        self.connected_clients = non_failed_clients;
    }
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

fn room_page(request: &Request, room_name: &str, history: Option<&VecDeque<Message>>) -> Response {
    let nickname = rouille::input::cookies(request)
        .find(|&(n, _)| n == "nickname")
        .map(|(_, v)| percent_decode(v.as_bytes()).decode_utf8_lossy().to_string());
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                link(rel="icon", type="image/vnd.microsoft.icon", href="/static/meowww.ico");
                link(rel="stylesheet", type="text/css", href="/static/style.css");
                script(src="/static/jquery.js") {}
                meta(name="viewport", content="width=device-width, initial-scale=1.0");
                meta(name="room_name", content=room_name);
                title : format!("Meowww - {}", room_name);
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
                        input(type="text", name="nickname", placeholder="Nickname", value?=nickname);
                        input(type="text", name="content", placeholder="Message content", autofocus);
                        input(type="submit", value="Send");
                    }
                }
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
    let url_encoded_nickname: String =
        utf8_percent_encode(&message.nickname, USERINFO_ENCODE_SET).collect();
    room.connect_clients();
    room.add_message(message);
    Response::text("Message sent. Please enable javascript for a better interface.")
        .with_additional_header(
            "Set-Cookie",
            format!("nickname=\"{}\"", url_encoded_nickname),
        )
}

fn create_notify_websocket(request: &Request, room: &mut Room) -> Response {
    use rouille::websocket;
    let (response, websocket_receiver) = try_or_400!(websocket::start(request, Some("meowww")));
    /* rouille::websocket:
     * start returns a response that must be sent before access to the websocket.
     * The current strategy is to store the mpsc::Receiver.
     * The socket is received later during a notification.
     */
    room.connect_clients();
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
