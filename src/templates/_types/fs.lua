-- .mockserver/fs.lua
---@meta

---@class fs
---Read-only file system access, sandboxed to the domain folder
local fs = {}

---Read a file's contents as a string
---@param path string Relative path to the file
---@return string contents The file contents
---@nodiscard
function fs.read(path) end

---Check if a file exists
---@param path string Relative path to check
---@return boolean exists True if the file exists
---@nodiscard
function fs.exists(path) end

return fs
