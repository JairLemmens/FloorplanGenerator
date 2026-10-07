use wasm_bindgen::prelude::*;

use crate::bim::building::Building;
use crate::bim::base_types::Id;
use super::base_types::WasmId;


#[wasm_bindgen]
pub struct WasmBuilding {
    binding: Building,
}

#[wasm_bindgen]
impl WasmBuilding {
    #[wasm_bindgen(constructor)]   
    #[wasm_bindgen(js_name = fromData)]
    pub fn from_json(topology_json: &str,construction_json: &str) -> Result<WasmBuilding, JsValue> {
        Building::from_json(topology_json, construction_json)
            .map(|binding| WasmBuilding { binding })
            .map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen(js_name = faceIds)]
    pub fn face_ids(&self) -> js_sys::Array {
        self.binding
            .topology
            .face_ids()
            .into_iter()
            .map(WasmId::from)
            .map(JsValue::from)
            .collect()
    }

    #[wasm_bindgen(js_name = layerGeometry)]
    pub fn layer_geometry(&self, face_id: WasmId) -> Result<JsValue, JsValue> {
        let face_id = Id {index: face_id.index, generation: face_id.generation};

        let geometry = self
            .binding
            .layer_geometry(face_id)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&geometry)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = solveJoints)]
    pub fn solve_joints(&mut self) {
        self.binding.solve_joints();
    }
}