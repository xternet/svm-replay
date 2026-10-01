use serde_json::json;
use std::fs;
use svm_replay_engine::shared::debug::{ExactSymbols, RuntimeFrames};
use svm_replay_protocol::Digest;

fn stripped_elf() -> Vec<u8> {
    let mut bytes = vec![0u8; 192];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&247u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[40..48].copy_from_slice(&64u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64u16.to_le_bytes());
    bytes[60..62].copy_from_slice(&2u16.to_le_bytes());
    bytes[62..64].copy_from_slice(&1u16.to_le_bytes());
    let names = b"\0.shstrtab\0";
    bytes[128..132].copy_from_slice(&1u32.to_le_bytes());
    bytes[132..136].copy_from_slice(&3u32.to_le_bytes());
    bytes[152..160].copy_from_slice(&192u64.to_le_bytes());
    bytes[160..168].copy_from_slice(&(names.len() as u64).to_le_bytes());
    bytes.extend_from_slice(names);
    bytes
}
#[test]
fn exact_symbol_admission_rejects_mismatch_and_reports_stripped_without_guessing() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("stripped.so");
    let bytes = stripped_elf();
    fs::write(&path, &bytes).unwrap();
    assert!(ExactSymbols::admit(&path, &Digest::of(b"foreign ELF")).is_err());
    let symbols = ExactSymbols::admit(&path, &Digest::of(&bytes)).unwrap();
    assert!(!symbols.available());
    assert_eq!(
        symbols.admission()["reason"],
        "missing embedded source DWARF"
    );
    assert!(symbols.location(0).is_err());
    let mut sbpf = bytes.clone();
    sbpf[18..20].copy_from_slice(&263u16.to_le_bytes());
    fs::write(&path, &sbpf).unwrap();
    assert!(!ExactSymbols::admit(&path, &Digest::of(&sbpf))
        .unwrap()
        .available());
    fs::write(&path, b"not an ELF").unwrap();
    assert!(ExactSymbols::admit(&path, &Digest::of(b"not an ELF")).is_err());
}
#[test]
fn runtime_frames_preserve_unsaved_caller_registers_and_explicit_truncation() {
    let mut current = vec![json!("0x0"); 11];
    current[10] = json!("0x200000000");
    let mut parent = vec![serde_json::Value::Null; 6];
    parent.extend([
        json!("0x6"),
        json!("0x7"),
        json!("0x8"),
        json!("0x9"),
        json!("0x200001000"),
    ]);
    let value = json!({"schema":"sbpf-runtime-frames/v1","depth":1,"truncated":false,"frames":[{"index":0,"pc":"0x120","framePointer":"0x200000000","registers":current},{"index":1,"pc":"0x180","framePointer":"0x200001000","registers":parent}]});
    let frames = RuntimeFrames::parse(&value).unwrap();
    assert_eq!(frames.depth, 1);
    assert!(frames.frames[1].registers[..6].iter().all(Option::is_none));
    let mut poison = value.clone();
    poison["frames"][1]["registers"][0] = json!("0x0");
    assert!(RuntimeFrames::parse(&poison).is_err());
    poison = value.clone();
    poison["depth"] = json!(8);
    assert!(RuntimeFrames::parse(&poison).is_err());
    poison = value;
    poison["depth"] = json!(64);
    poison["truncated"] = json!(true);
    assert!(
        RuntimeFrames::parse(&poison).is_err(),
        "truncated endpoint must provide its complete bounded64frame prefix"
    );
}
