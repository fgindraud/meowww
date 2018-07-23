#[macro_use]
extern crate rouille;
#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate rust_embed;

use horrorshow::Template;
use rouille::Response;

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
    rouille::start_server(addr, move |request| {
        router!(request,
            (GET) ["/"] => { home_page() },
            (GET) ["/static/{asset}", asset: String] => { send_asset(&asset) },
            (GET) ["/{room}", room: String] => { room_page(&room) },
            _ => { Response::empty_404() }
        )
    })
}

fn room_page(name: &str) -> Response {
    let template = html! {
        : horrorshow::helper::doctype::HTML;
        html {
            head {
                link(rel="stylesheet", type="text/css", href="/static/style.css");
                title : format!("Meowww - {}", name);
            }
            body {
                main {
                    table {
                    }
                }
                footer {
                    form(autocomplete="off") {
                        input(type="text", name="name", placeholder="Name");
                        input(type="text", name="message", placeholder="Message", autofocus);
                        input(type="submit", value="Send", disabled);
                    }
                }
                script(src="/static/jquery.js") {}
                script(src="/static/client.js") {}
            }
        }
    };
    Response::html(template.into_string().unwrap())
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
