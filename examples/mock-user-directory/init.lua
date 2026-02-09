-- mock-user-directory: Mock an Auth0/Okta user directory API for apps that
-- look up users and validate tokens.
--
-- Try:
--   curl -H "Host: mock-user-directory" localhost:3000/v1/users
--   curl -X POST -H "Host: mock-user-directory" -H "Content-Type: application/json" \
--     -d '{"name":"Alice","email":"alice@example.com"}' localhost:3000/v1/users

local json = require("json")
local state = require("state")
local uuid = require("uuid")

local VALID_TOKENS = {
    ["tok_admin"] = { user_id = "seed-1", role = "admin" },
    ["tok_viewer"] = { user_id = "seed-2", role = "viewer" },
}

local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data),
    }
end

local function get_users()
    local users = state.get("users")
    if not users then
        users = {
            ["seed-1"] = { id = "seed-1", name = "Admin User", email = "admin@example.com", role = "admin" },
            ["seed-2"] = { id = "seed-2", name = "Viewer User", email = "viewer@example.com", role = "viewer" },
        }
        state.set("users", users)
    end
    return users
end

---@param request Request
---@return Response
function handle(request)
    -- POST /v1/auth/validate — validate token
    if request.method == "POST" and request.path == "/v1/auth/validate" then
        local data = json.decode(request.body)
        local token_info = VALID_TOKENS[data.token]
        if not token_info then
            return json_response(401, { error = "Invalid token" })
        end
        return json_response(200, { valid = true, user_id = token_info.user_id, role = token_info.role })
    end

    local users = get_users()
    local user_id = request.path:match("^/v1/users/([^/]+)$")

    -- GET /v1/users — list all users
    if request.method == "GET" and request.path == "/v1/users" then
        local list = {}
        for _, user in pairs(users) do
            table.insert(list, user)
        end
        return json_response(200, { users = list, total = #list })
    end

    -- GET /v1/users/:id — get user
    if request.method == "GET" and user_id then
        if users[user_id] then
            return json_response(200, users[user_id])
        end
        return json_response(404, { error = "User not found" })
    end

    -- POST /v1/users — create user
    if request.method == "POST" and request.path == "/v1/users" then
        local data = json.decode(request.body)
        local id = uuid.v4()
        local user = {
            id = id,
            name = data.name,
            email = data.email,
            role = data.role or "viewer",
        }
        users[id] = user
        state.set("users", users)
        return json_response(201, user)
    end

    -- DELETE /v1/users/:id — remove user
    if request.method == "DELETE" and user_id then
        if not users[user_id] then
            return json_response(404, { error = "User not found" })
        end
        users[user_id] = nil
        state.set("users", users)
        return { status = 204 }
    end

    return json_response(404, { error = "Not found" })
end
