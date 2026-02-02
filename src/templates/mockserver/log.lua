-- .mockserver/log.lua
---@meta

---@class log
local log = {}

---Log a debug message (only visible with -v flag)
---@param message string The message to log
function log.debug(message) end

---Log an info message
---@param message string The message to log
function log.info(message) end

---Log a warning message
---@param message string The message to log
function log.warn(message) end

---Log an error message
---@param message string The message to log
function log.error(message) end

return log
