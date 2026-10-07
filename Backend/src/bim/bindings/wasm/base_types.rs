use crate::bim::base_types::Id;
use wasm_bindgen::prelude::*;


#[wasm_bindgen]
pub struct WasmId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

#[wasm_bindgen]
impl WasmId {
    #[wasm_bindgen(constructor)]
    pub fn new(index: u32, generation: u32) -> WasmId {
        WasmId { index, generation }
    }

    #[wasm_bindgen(getter)]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[wasm_bindgen(getter)]
    pub fn generation(&self) -> u32 {
        self.generation
    }
}

impl From<Id> for WasmId {
    fn from(id: Id) -> Self {
        Self {
            index: id.index,
            generation: id.generation,
        }
    }
}