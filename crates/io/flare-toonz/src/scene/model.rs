/// A parsed `.tnz` scene: the parts a Flare project needs to know about, typed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// The `version` attribute of `<tnz>`, e.g. `71.0`.
    pub version: Option<String>,
    pub generator: Option<String>,
    /// Frame rate of the main output (`properties/outputs/output/fps`).
    pub frame_rate: Option<f64>,
    pub camera_count: usize,
    /// Pixel size of the first camera (`cameraRes`), when the scene has one.
    pub camera_res: Option<(u32, u32)>,
    /// Size of the first camera in inches (`cameraSize`).
    pub camera_size: Option<(f64, f64)>,
    /// Every level declared in the level set, in file order.
    pub levels: Vec<LevelDecl>,
    /// Xsheet columns, in file order.
    pub columns: Vec<Column>,
}

impl Scene {
    /// Rows used by the xsheet: one past the last cell of any level or palette column.
    pub fn frame_count(&self) -> u32 {
        self.columns
            .iter()
            .filter_map(|column| match column {
                Column::Level(cells) | Column::Palette(cells) => cells.last(),
                Column::Sound(_) | Column::Other { .. } => None,
            })
            .map(|run| run.row + run.count)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelDecl {
    /// The persistence id that cells and sound columns refer to.
    pub id: u32,
    pub name: String,
    /// The `type` attribute as written (for example `vector`, `raster`, `sound`).
    pub kind: String,
    /// The `path` child as written, e.g. `+drawings/A.pli`; not resolved on disk.
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    /// A level column; its cells refer to levels.
    Level(Vec<CellRun>),
    /// A palette column; its cells refer to palette levels.
    Palette(Vec<CellRun>),
    /// A sound column.
    Sound(SoundCells),
    /// A column kind not decoded yet (its tag name is kept).
    Other { tag: String },
}

/// `count` consecutive rows from `row` that show frame `frame` of level `level`, the frame
/// number moving by `increment` each row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRun {
    pub row: u32,
    pub count: u32,
    pub level: u32,
    pub frame: FrameId,
    pub increment: i32,
}

/// A frame number, with the optional letter Toonz uses for drawings like `12a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameId {
    pub number: u32,
    pub letter: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundCells {
    pub start_offset: u32,
    pub end_offset: u32,
    pub start_frame: u32,
    pub level: u32,
}
