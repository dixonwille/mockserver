// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Sandboxed filesystem module (read, exists)

use mlua::{Lua, Result as LuaResult};
use std::path::{Path, PathBuf};

use super::preload_module;

pub fn register(lua: &Lua, domain_dir: &Path) -> LuaResult<()> {
    let fs = lua.create_table()?;
    let base_dir = domain_dir.to_path_buf();

    let base_clone = base_dir.clone();
    // fs.read(path) -> string
    fs.set(
        "read",
        lua.create_function(move |_, path: String| {
            let resolved = resolve_sandboxed_path(&base_clone, &path)?;
            std::fs::read_to_string(&resolved)
                .map_err(|e| mlua::Error::external(format!("Failed to read file: {e}")))
        })?,
    )?;

    // fs.exists(path) -> boolean
    fs.set(
        "exists",
        lua.create_function(move |_, path: String| {
            match resolve_sandboxed_path(&base_dir, &path) {
                Ok(resolved) => Ok(resolved.exists()),
                Err(_) => Ok(false), // Path traversal = doesn't exist
            }
        })?,
    )?;

    preload_module(lua, "fs", fs)?;
    Ok(())
}

/// Resolve a path relative to base_dir, ensuring it doesn't escape the sandbox
fn resolve_sandboxed_path(base_dir: &Path, relative_path: &str) -> LuaResult<PathBuf> {
    // Block absolute paths
    if relative_path.starts_with('/') || relative_path.starts_with('\\') {
        return Err(mlua::Error::external("Absolute paths are not allowed"));
    }

    // Block explicit path traversal
    if relative_path.contains("..") {
        return Err(mlua::Error::external("Path traversal is not allowed"));
    }

    let joined = base_dir.join(relative_path);

    // Canonicalize to resolve any symlinks and verify final path
    let canonical = joined
        .canonicalize()
        .map_err(|e| mlua::Error::external(format!("Cannot resolve path: {e}")))?;

    let base_canonical = base_dir
        .canonicalize()
        .map_err(|e| mlua::Error::external(format!("Cannot resolve base path: {e}")))?;

    // Ensure resolved path is within base directory
    if !canonical.starts_with(&base_canonical) {
        return Err(mlua::Error::external(
            "Access denied: path outside domain folder",
        ));
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_fs_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        let result: String = lua
            .load(r#"return require("fs").read("test.txt")"#)
            .eval()
            .unwrap();

        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_fs_read_nonexistent() {
        let temp_dir = TempDir::new().unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        let result: Result<String, _> = lua
            .load(r#"return require("fs").read("nonexistent.txt")"#)
            .eval();

        assert!(result.is_err());
    }

    #[test]
    fn test_fs_exists_true() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("exists.txt");
        std::fs::write(&test_file, "content").unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        let result: bool = lua
            .load(r#"return require("fs").exists("exists.txt")"#)
            .eval()
            .unwrap();

        assert!(result);
    }

    #[test]
    fn test_fs_exists_false() {
        let temp_dir = TempDir::new().unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        let result: bool = lua
            .load(r#"return require("fs").exists("nonexistent.txt")"#)
            .eval()
            .unwrap();

        assert!(!result);
    }

    #[test]
    fn test_fs_exists_traversal_returns_false() {
        let temp_dir = TempDir::new().unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        // Path traversal should return false, not error
        let result: bool = lua
            .load(r#"return require("fs").exists("../../../etc/passwd")"#)
            .eval()
            .unwrap();

        assert!(!result);
    }

    #[test]
    fn test_fs_read_nested_file() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("data");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("file.json"), r#"{"key":"value"}"#).unwrap();

        let lua = Lua::new();
        register(&lua, temp_dir.path()).unwrap();

        let result: String = lua
            .load(r#"return require("fs").read("data/file.json")"#)
            .eval()
            .unwrap();

        assert_eq!(result, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_sandboxed_path_blocks_traversal() {
        let base = Path::new("/mocks/api.example.com");

        // These should fail
        assert!(resolve_sandboxed_path(base, "../other.com/init.lua").is_err());
        assert!(resolve_sandboxed_path(base, "/etc/passwd").is_err());
        assert!(resolve_sandboxed_path(base, "foo/../../bar").is_err());
    }

    #[test]
    fn test_sandboxed_path_blocks_absolute() {
        let temp_dir = TempDir::new().unwrap();

        let result = resolve_sandboxed_path(temp_dir.path(), "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Absolute paths"));
    }

    #[test]
    fn test_sandboxed_path_blocks_backslash_absolute() {
        let temp_dir = TempDir::new().unwrap();

        let result = resolve_sandboxed_path(temp_dir.path(), "\\windows\\system32");
        assert!(result.is_err());
    }

    #[test]
    fn test_sandboxed_path_allows_valid() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("valid.lua");
        std::fs::write(&test_file, "test").unwrap();

        let result = resolve_sandboxed_path(temp_dir.path(), "valid.lua");
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandboxed_path_allows_nested() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("routes");
        std::fs::create_dir_all(&subdir).unwrap();
        let test_file = subdir.join("users.lua");
        std::fs::write(&test_file, "test").unwrap();

        let result = resolve_sandboxed_path(temp_dir.path(), "routes/users.lua");
        assert!(result.is_ok());
    }
}
