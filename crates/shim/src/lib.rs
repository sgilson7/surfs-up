//! surf_shim: moves values between the page and `surf_core::game`.
//!
//! Builder requirement: the shim decides nothing. Each function is one call
//! into the core and a conversion of its arguments or result.
//! `crates/core/tests/boundary.rs` fails if a branch, a loop or the `?`
//! operator appears here.

use surf_core::game;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn numbers() -> String {
    game::numbers()
}

#[wasm_bindgen]
pub fn noise(samples: f64, seed: u32) -> Vec<f32> {
    game::noise(samples as u32, seed)
}

#[wasm_bindgen]
pub struct Game {
    inner: game::Game,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(maps: &str, saved: &str) -> Result<Game, JsValue> {
        game::Game::new(maps, saved)
            .map(|inner| Game { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    pub fn map_wanted(&self) -> String {
        self.inner.map_wanted()
    }

    pub fn map_loaded(&self) -> String {
        self.inner.map_loaded()
    }

    pub fn choose_map(&mut self, id: &str) -> bool {
        self.inner.choose_map(id)
    }

    pub fn load_map(&mut self, id: &str, bytes: &[u8]) -> Result<(), JsValue> {
        self.inner.load_map(id, bytes).map_err(|e| JsValue::from_str(&e))
    }

    /// CSS size and device pixel ratio in; drawing-buffer size out.
    pub fn resize(&mut self, css_w: f64, css_h: f64, dpr: f64) -> Vec<u32> {
        self.inner.resize(css_w as f32, css_h as f32, dpr as f32).to_vec()
    }

    /// The page's clock in milliseconds, held action bits, mouse movement in CSS pixels.
    pub fn advance(&mut self, now_ms: f64, bits: u32, dx: f64, dy: f64) {
        self.inner.advance(now_ms, bits, dx, dy)
    }

    pub fn world_vertices(&self) -> Vec<f32> {
        self.inner.world_vertices().to_vec()
    }

    pub fn world_counts(&self) -> Vec<u32> {
        self.inner.world_counts().to_vec()
    }

    pub fn view(&self) -> Vec<f32> {
        self.inner.view()
    }

    pub fn others_vertices(&self) -> Vec<f32> {
        self.inner.others_vertices().to_vec()
    }

    pub fn others_opaque(&self) -> u32 {
        self.inner.others_opaque()
    }

    pub fn hands_vertices(&self) -> Vec<f32> {
        self.inner.hands_vertices().to_vec()
    }

    pub fn labels(&self) -> String {
        self.inner.labels()
    }

    pub fn hud(&self) -> String {
        self.inner.hud()
    }

    pub fn screen(&self) -> String {
        self.inner.screen()
    }

    pub fn events(&mut self) -> String {
        self.inner.events()
    }

    pub fn saved(&self) -> String {
        self.inner.saved()
    }

    pub fn play(&mut self) {
        self.inner.play()
    }

    pub fn pause(&mut self) {
        self.inner.pause()
    }

    pub fn to_title(&mut self) {
        self.inner.to_title()
    }

    pub fn set_name(&mut self, name: &str) {
        self.inner.set_name(name)
    }

    pub fn cycle_shirt(&mut self) {
        self.inner.cycle_shirt()
    }

    pub fn hold(&mut self, id: &str) -> bool {
        self.inner.hold(id)
    }

    pub fn next_held(&mut self) {
        self.inner.next_held()
    }

    pub fn set_setting(&mut self, key: &str, value: f64) {
        self.inner.set_setting(key, value)
    }

    pub fn nudge(&mut self, key: &str, up: bool) {
        self.inner.nudge(key, up)
    }

    pub fn host_online(&mut self, seed: f64) -> String {
        self.inner.host_online(seed)
    }

    pub fn join_online(&mut self, code: &str) -> bool {
        self.inner.join_online(code)
    }

    pub fn leave_online(&mut self) {
        self.inner.leave_online()
    }

    pub fn room_code(&self) -> String {
        self.inner.room_code()
    }

    pub fn net_peer(&mut self, id: &str, host: bool) {
        self.inner.net_peer(id, host)
    }

    pub fn net_left(&mut self, id: &str) {
        self.inner.net_left(id)
    }

    pub fn net_error(&mut self, key: &str) {
        self.inner.net_error(key)
    }

    pub fn net_message(&mut self, id: &str, text: &str) {
        self.inner.net_message(id, text)
    }

    pub fn net_outbox(&mut self) -> String {
        self.inner.net_outbox()
    }

    pub fn dev_at(&mut self, text: &str) {
        self.inner.dev_at(text)
    }
}
