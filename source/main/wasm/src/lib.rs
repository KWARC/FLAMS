#[wasm_bindgen::prelude::wasm_bindgen]
pub fn main() {
    flams_main::hydrate()
}

#[cfg(any(doc, feature = "docs"))]
pub mod endpoints;
