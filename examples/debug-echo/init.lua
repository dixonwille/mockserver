-- debug-echo: Diagnostic tool — point your app at this mock to inspect
-- exactly what requests it sends to a third-party API.
--
-- Try:
--   curl -H "Host: debug-echo" localhost:3000/any/path?foo=bar

local json = require("json")
local time = require("time")

---@param request Request
---@return Response
function handle(request)
    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            method = request.method,
            path = request.path,
            query = request.query,
            headers = request.headers,
            body = request.body,
            domain = request.domain,
            received_at = time.iso8601(),
        }),
    }
end
