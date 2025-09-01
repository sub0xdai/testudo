use leptos::*;
use wasm_bindgen::prelude::*;

mod app;
use app::App;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    
    mount_to_body(|| {
        view! {
            <App/>
        }
    });
}