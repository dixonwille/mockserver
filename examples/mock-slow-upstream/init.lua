-- mock-slow-upstream: Mock a slow image processing API to test your app's
-- timeout handling and loading states.
--
-- Try:
--   curl -X POST -H "Host: mock-slow-upstream" localhost:3000/v1/process
--   curl -H "Host: mock-slow-upstream" localhost:3000/v1/status/job-123
--   curl -H "Host: mock-slow-upstream" localhost:3000/v1/health

local json = require("json")
local delay = require("delay")
local time = require("time")

local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data),
    }
end

---@param request Request
---@return Response
function handle(request)
    -- POST /v1/process — slow processing endpoint (2s delay)
    if request.method == "POST" and request.path == "/v1/process" then
        delay.sleep(2000)
        return json_response(200, {
            status = "completed",
            processed_at = time.iso8601(),
            result = { width = 800, height = 600, format = "png" },
        })
    end

    -- GET /v1/status/:id — poll endpoint (returns immediately)
    local job_id = request.path:match("^/v1/status/([^/]+)$")
    if request.method == "GET" and job_id then
        return json_response(200, {
            job_id = job_id,
            status = "processing",
            progress = 42,
        })
    end

    -- GET /v1/health — health check (returns immediately)
    if request.method == "GET" and request.path == "/v1/health" then
        return json_response(200, { status = "healthy" })
    end

    return json_response(404, { error = "Not found" })
end
