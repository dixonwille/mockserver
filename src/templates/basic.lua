-- {{DOMAIN}}/init.lua
-- Basic mock handler

local json = require("json")

---@param request Request
---@return Response
function handle(request)
    -- Echo the request back as JSON
    ---@type Response
    return {
        status = 200,
        headers = {
            ["Content-Type"] = "application/json"
        },
        body = json.encode({
            message = "Hello from {{DOMAIN}}",
            method = request.method,
            path = request.path
        })
    }
end
