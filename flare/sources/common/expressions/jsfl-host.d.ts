// Engine globals the Flare script host (ac_script.rs) provides to the JSFL prelude.
// `ac` is the untyped command bridge (ac.doc, ac.frame, ac.selection, ...); its
// shape is defined by the Rust side, so it is typed `any` here.
declare var ac: any;
declare function print(...args: any[]): void;
