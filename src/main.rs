#[macro_use]
extern crate rouille;
#[macro_use]
extern crate horrorshow;
#[macro_use]
extern crate clap;

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
            _ => { Response::empty_404() }
        )
    })
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
