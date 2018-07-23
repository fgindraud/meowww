#[macro_use]
extern crate rouille;
#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate rust_embed;

use horrorshow::Template;
use rouille::{Request, Response};
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

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
    let rooms = RwLock::new(HashMap::<String, Room>::new());
    rouille::start_server(addr, move |request| {
        router!(request,
            (GET) ["/"] => { home_page() },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room}", room: String] => {
                room_page(&room, rooms.read().unwrap().get(&room).map(|r : &Room| r.history()))
            },
            (POST) ["/{room}", room: String] => {
                post_message(
                    request,
                    rooms.write().unwrap().entry (room.clone()).or_insert_with(|| Room::new(room))
                    )
            },
            (GET) ["/{room}/notify", room: String] => {
                create_notify_websocket(request, &room)
            },
            _ => { Response::empty_404() }
        )
    })
}

struct Message {
    nickname: String,
    content: String,
}

// TODO history limit
struct Room {
    name: String,
    history: VecDeque<Message>,
}

impl Room {
    fn new<S: Into<String>>(name: S) -> Self {
        Room {
            name: name.into(),
            history: VecDeque::new(),
        }
    }
    fn name(&self) -> &String {
        &self.name
    }
    fn add_message(&mut self, message: Message) {
        self.history.push_back(message)
    }
    fn history(&self) -> &VecDeque<Message> {
        &self.history
    }
}

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

fn create_notify_websocket(request: &Request, room: &str) -> Response {
    use rouille::websocket;
    let (response, websocket) = try_or_400!(websocket::start(request, Some("meowww")));

    std::thread::spawn(move || {
        let mut ws = websocket.recv().unwrap();
    });

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
