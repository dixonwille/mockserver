-- mock-feature-flags: Mock a LaunchDarkly-style feature flag service to
-- control which features are enabled in tests.
--
-- Try:
--   curl -H "Host: mock-feature-flags" localhost:3000/v1/flags
--   curl -X PUT -H "Host: mock-feature-flags" -H "Content-Type: application/json" \
--     -d '{"enabled":true}' localhost:3000/v1/flags/dark-mode

local json = require("json")
local fs = require("fs")
local state = require("state")

local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data),
    }
end

local function load_flags()
    local flags = state.get("flags")
    if not flags then
        flags = json.decode(fs.read("fixtures/flags.json"))
        state.set("flags", flags)
    end
    return flags
end

---@param request Request
---@return Response
function handle(request)
    -- POST /v1/flags/reset — reset to fixture defaults
    if request.method == "POST" and request.path == "/v1/flags/reset" then
        state.clear()
        local flags = json.decode(fs.read("fixtures/flags.json"))
        state.set("flags", flags)
        return json_response(200, { reset = true, flags = flags })
    end

    -- GET /v1/flags — list all flags
    if request.method == "GET" and request.path == "/v1/flags" then
        return json_response(200, { flags = load_flags() })
    end

    -- GET /v1/flags/:key — single flag
    local key = request.path:match("^/v1/flags/([^/]+)$")
    if request.method == "GET" and key then
        local flags = load_flags()
        if flags[key] ~= nil then
            return json_response(200, { key = key, value = flags[key] })
        end
        return json_response(404, { error = "Flag not found: " .. key })
    end

    -- PUT /v1/flags/:key — override a flag
    if request.method == "PUT" and key then
        local flags = load_flags()
        local data = json.decode(request.body)
        flags[key] = data.enabled
        state.set("flags", flags)
        return json_response(200, { key = key, value = flags[key] })
    end

    return json_response(404, { error = "Not found" })
end
