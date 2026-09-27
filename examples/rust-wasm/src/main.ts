import init from "./module.wasm?wasm";
import { hash } from "rust:my_crypto";

const wasm = await init();
console.log("wasm exports:", Object.keys(wasm));
console.log("rust hash:", hash("ferrite"));
