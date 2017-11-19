extern crate dmt;

use dmt::*;

fn main(){

    let mut tr = TemplateRenderer::default();

    match tr.render() {
        Err(e) => {
            eprintln!("{}, ", e);
            for e in e.iter().skip(1) {
                eprintln!("{}", e);
            }
            ::std::process::exit(1);
        }
        _ => (),
    }
}