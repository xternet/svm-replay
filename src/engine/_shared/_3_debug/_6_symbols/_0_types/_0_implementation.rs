use super::*;

pub(in super::super) type DwarfReader = EndianArcSlice<LittleEndian>;

pub(in super::super) fn dwarf_error(error: impl std::fmt::Display) -> Error {
    super::super::super::error("SYMBOL_FORMAT", error.to_string())
}

pub(in super::super) fn address(text: &str) -> Result<u64, Error> {
    require(
        text.starts_with("0x") && (3..=18).contains(&text.len()),
        "DEBUG_FRAMES",
        "invalid runtime hexadecimal address",
    )?;
    let value = u64::from_str_radix(&text[2..], 16).map_err(dwarf_error)?;
    require(
        format!("0x{value:x}") == text,
        "DEBUG_FRAMES",
        "noncanonical runtime hexadecimal address",
    )?;
    Ok(value)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeFrame {
    pub index: u64,
    pub pc: String,
    pub frame_pointer: String,
    pub registers: Vec<Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeFrames {
    pub schema: String,
    pub depth: u64,
    pub truncated: bool,
    pub frames: Vec<RuntimeFrame>,
}

impl RuntimeFrames {
    pub fn parse(value: &Value) -> Result<Self, Error> {
        let frames: Self = serde_json::from_value(value.clone()).map_err(dwarf_error)?;
        require(
            frames.schema == "sbpf-runtime-frames/v1"
                && frames.depth < 65535
                && !frames.frames.is_empty()
                && frames.frames.len() <= 64,
            "DEBUG_FRAMES",
            "unknown or oversized runtime frames",
        )?;
        require(
            if frames.truncated {
                frames.depth >= 64 && frames.frames.len() == 64
            } else {
                frames.depth < 64 && frames.frames.len() as u64 == frames.depth + 1
            },
            "DEBUG_FRAMES",
            "runtime frame count/depth/truncation differs",
        )?;
        for (index, frame) in frames.frames.iter().enumerate() {
            require(
                frame.index == index as u64 && frame.registers.len() == 11,
                "DEBUG_FRAMES",
                "runtime frame index/register count differs",
            )?;
            address(&frame.pc)?;
            address(&frame.frame_pointer)?;
            for (register, value) in frame.registers.iter().enumerate() {
                if index > 0 && register < 6 {
                    require(
                        value.is_none(),
                        "DEBUG_FRAMES",
                        "unsaved caller scratch register fabricated",
                    )?;
                } else {
                    address(
                        value.as_deref().ok_or_else(|| {
                            error("DEBUG_FRAMES", "saved/current register missing")
                        })?,
                    )?;
                }
            }
            require(
                frame.registers[10].as_deref() == Some(&frame.frame_pointer),
                "DEBUG_FRAMES",
                "frame pointer differs from saved r10",
            )?;
        }
        Ok(frames)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub function: Option<String>,
}

pub(in super::super) struct Variable {
    pub(in super::super) name: String,
    pub(in super::super) offset: gimli::UnitOffset<usize>,
    pub(in super::super) direct_scope: bool,
}

pub(in super::super) struct Function {
    pub(in super::super) unit: usize,
    pub(in super::super) ranges: Vec<gimli::Range>,
    pub(in super::super) frame_base: Option<gimli::Expression<DwarfReader>>,
    pub(in super::super) variables: Vec<Variable>,
    pub(in super::super) prologue_end: Option<u64>,
}

pub(in super::super) struct Line {
    pub(in super::super) address: u64,
    pub(in super::super) file: String,
    pub(in super::super) line: u32,
}

pub struct ExactSymbols {
    pub(in super::super) path: PathBuf,
    pub(in super::super) elf_sha256: Digest,
    pub(in super::super) available: bool,
    pub(in super::super) dwarf: Option<Arc<gimli::Dwarf<DwarfReader>>>,
    pub(in super::super) context: Option<addr2line::Context<DwarfReader>>,
    pub(in super::super) units: Vec<gimli::Unit<DwarfReader>>,
    pub(in super::super) functions: Vec<Function>,
    pub(in super::super) lines: Vec<Line>,
}

pub(in super::super) fn text(
    dwarf: &gimli::Dwarf<DwarfReader>,
    unit: &gimli::Unit<DwarfReader>,
    value: AttributeValue<DwarfReader>,
) -> Result<String, Error> {
    let value = dwarf.attr_string(unit, value).map_err(dwarf_error)?;
    require(
        value.len() <= 4096,
        "SYMBOL_LIMIT",
        "DWARF string exceeds display bound",
    )?;
    Ok(value.to_string_lossy().map_err(dwarf_error)?.into_owned())
}
