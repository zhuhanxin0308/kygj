//! 在任何文件系统写入前验证跨平台目录名称，避免项目逃逸与覆盖。

use workbench_core::validation::validate_project_name;

#[test]
fn accepts_readable_single_directory_names() {
    for name in ["Ellis 光线研究", "wormhole-v1", "研究_2026"] {
        assert!(validate_project_name(name).is_ok(), "合法名称被拒绝：{name}");
    }
}

#[test]
fn rejects_traversal_separators_and_absolute_paths() {
    for name in ["..", ".", "../escape", "a/b", "a\\b", "C:\\escape", "/root"] {
        assert!(validate_project_name(name).is_err(), "禁止的路径被接受：{name}");
    }
}

#[test]
fn rejects_windows_reserved_names_even_with_extensions() {
    for name in ["CON", "con.txt", "NUL", "PRN", "AUX", "COM1", "lpt9.log"] {
        assert!(validate_project_name(name).is_err(), "保留名被接受：{name}");
    }
}

#[test]
fn rejects_ambiguous_suffixes_control_characters_and_empty_names() {
    for name in ["", " ", "name.", "name ", "a\nb", "a:b", "a?b", "a*b"] {
        assert!(validate_project_name(name).is_err(), "非法名称被接受：{name:?}");
    }
}
