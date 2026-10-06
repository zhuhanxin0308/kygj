use crate::{CoreError, CoreResult};

pub const MAX_PROJECT_NAME_BYTES: usize = 255;

/// 使用三平台共同允许的单目录名称，拒绝系统会归一化或解释为路径的输入。
pub fn validate_project_name(name: &str) -> CoreResult<()> {
    let invalid = name.is_empty()
        || name.trim() != name
        || name.len() > MAX_PROJECT_NAME_BYTES
        || matches!(name, "." | "..")
        || name.ends_with('.')
        || name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c));
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"))
        });
    if invalid || reserved {
        return Err(CoreError::new("invalid_project_name", "项目名称必须是合法的单个目录名，不能包含路径、保留名或尾随空白和点"));
    }
    Ok(())
}
