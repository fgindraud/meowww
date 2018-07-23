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
use std::collections::HashMap;
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
    let database = RwLock::new(Database::new());
    rouille::start_server(addr, move |request| {
        router!(request,
            (GET) ["/"] => { home_page() },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room}", room: String] => {
                room_page(&room, database.read().unwrap().get_history(&room))
            },
            (POST) ["/{room}", room: String] => {
                post_message(request, &room, &mut database.write().unwrap())
            },
            _ => { Response::empty_404() }
        )
    })
}

struct Message {
    nickname: String,
    content: String,
}

struct Database {
    history: HashMap<String, Vec<Message>>,
}
impl Database {
    fn new() -> Self {
        Database {
            history: HashMap::new(),
        }
    }
    fn add_message(&mut self, room: &str, message: Message) {
        self.history
            .entry(room.to_owned())
            .or_insert_with(|| Vec::new())
            .push(message)
    }
    fn get_history(&self, room: &str) -> Option<&[Message]> {
        self.history.get(room).map(|v| v.as_slice())
    }
}

fn room_page(room: &str, history: Option<&[Message]>) -> Response {
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
                            @ for m in messages {
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

fn post_message(request: &Request, room: &str, database: &mut Database) -> Response {
    let form_data = try_or_400!(post_input!(request, { nickname: String, content: String }));
    let message = Message {
        nickname: form_data.nickname,
        content: form_data.content,
    };
    database.add_message(room, message);
    Response::redirect_303(format!("/{}", room))
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
