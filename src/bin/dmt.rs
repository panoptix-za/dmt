extern crate dmt;

use dmt::*;

fn main() {
    let mut tr = TemplateRenderer::default();

    match tr.render_default() {
        Err(e) => {
            eprintln!("{:?}", e);
            ::std::process::exit(1);
        }
        _ => (),
    }

    match tr.render_multipart() {
        Err(e) => {
            eprintln!("{:?}", e);
            ::std::process::exit(1);
        }
        _ => (),
    }
}
