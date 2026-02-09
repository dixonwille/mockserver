-- mock-notification-service: Mock a SendGrid/Twilio notification API to
-- capture what your app would send without actually sending.
--
-- Try:
--   curl -X POST -H "Host: mock-notification-service" -H "Content-Type: application/json" \
--     -d '{"to":"user@example.com","subject":"Hello","body":"Hi there","channel":"email"}' \
--     localhost:3000/v1/messages
--   curl -H "Host: mock-notification-service" localhost:3000/v1/messages

local json = require("json")
local state = require("state")
local time = require("time")
local log = require("log")

local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data),
    }
end

local function get_messages()
    return state.get("messages") or {}
end

local function next_id()
    local seq = (state.get("seq") or 0) + 1
    state.set("seq", seq)
    return "msg_" .. seq
end

---@param request Request
---@return Response
function handle(request)
    local messages = get_messages()

    -- POST /v1/messages — send a message (captured, not actually sent)
    if request.method == "POST" and request.path == "/v1/messages" then
        local data = json.decode(request.body)
        local id = next_id()
        local msg = {
            id = id,
            to = data.to,
            subject = data.subject,
            body = data.body,
            channel = data.channel or "email",
            sent_at = time.iso8601(),
        }
        messages[id] = msg
        state.set("messages", messages)
        log.info("Captured message " .. id .. " to " .. (data.to or "unknown"))
        return json_response(201, msg)
    end

    -- GET /v1/messages — list all captured messages
    if request.method == "GET" and request.path == "/v1/messages" then
        local list = {}
        for _, msg in pairs(messages) do
            table.insert(list, msg)
        end
        return json_response(200, { messages = list, total = #list })
    end

    -- GET /v1/messages/:id — get a specific message
    local msg_id = request.path:match("^/v1/messages/([^/]+)$")
    if request.method == "GET" and msg_id then
        if messages[msg_id] then
            return json_response(200, messages[msg_id])
        end
        return json_response(404, { error = "Message not found" })
    end

    -- DELETE /v1/messages — clear all messages
    if request.method == "DELETE" and request.path == "/v1/messages" then
        state.clear()
        return json_response(200, { cleared = true })
    end

    return json_response(404, { error = "Not found" })
end
