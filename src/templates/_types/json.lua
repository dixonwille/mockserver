-- .mockserver/json.lua
---@meta

---@class json
local json = {}

---Encode a Lua value to a JSON string
---@param value any The value to encode
---@return string json The JSON-encoded string
---@nodiscard
function json.encode(value) end

---Decode a JSON string to a Lua value
---@param str string The JSON string to decode
---@return any value The decoded Lua value
---@nodiscard
function json.decode(str) end

return json
