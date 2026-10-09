use std::path::PathBuf;

use ratatui::buffer::Buffer;

pub fn maybe_write_render_dump(buffer: &Buffer) {
    let Some(path) = dump_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let width = usize::from(buffer.area.width);
    let height = usize::from(buffer.area.height);
    let content = buffer.content();
    let mut lines = Vec::with_capacity(height);
    for row in 0..height {
        let start = row * width;
        let end = start + width;
        let mut line = String::new();
        for cell in &content[start..end] {
            line.push_str(cell.symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    let text = lines.join("\n");
    let _ = std::fs::write(path, text);
}

fn dump_path() -> Option<PathBuf> {
    std::env::var_os("CZAR_TUI_RENDER_DUMP_PATH").map(PathBuf::from)
}
