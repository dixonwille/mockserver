# Lua Scripting Examples

This document provides comprehensive examples of Lua scripting patterns for the mock server. Each example is complete and runnable.

## Table of Contents

- [REST Patterns](#rest-patterns)
  - [Basic CRUD Endpoints](#basic-crud-endpoints)
  - [Offset-Based Pagination](#offset-based-pagination)
  - [Cursor-Based Pagination](#cursor-based-pagination)
  - [Filtering and Search](#filtering-and-search)
  - [Versioned APIs](#versioned-apis)
- [Response Simulation](#response-simulation)
  - [Error Responses](#error-responses)
  - [Slow Responses](#slow-responses)
  - [Conditional Responses](#conditional-responses)
- [Authentication Mocking](#authentication-mocking)
  - [Bearer Token Validation](#bearer-token-validation)
  - [API Key Checking](#api-key-checking)
  - [OAuth Token Exchange](#oauth-token-exchange)
- [Stateful Flows](#stateful-flows)
  - [Shopping Cart](#shopping-cart)
  - [Multi-Step Wizard](#multi-step-wizard)
  - [Rate Limiting Simulation](#rate-limiting-simulation)
  - [Request Counting](#request-counting)
- [GraphQL Patterns](#graphql-patterns)
  - [Query Handling](#query-handling)
  - [Mutation Responses](#mutation-responses)
  - [Resolver-Style Routing](#resolver-style-routing)
- [Advanced Patterns](#advanced-patterns)
  - [Reading Fixture Files](#reading-fixture-files)
  - [Sharing State Across Requests](#sharing-state-across-requests)
  - [Dynamic Response Generation](#dynamic-response-generation)
  - [Logging for Debugging](#logging-for-debugging)

---

## REST Patterns

### Basic CRUD Endpoints

Complete REST API with GET, POST, PUT, and DELETE operations.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")

---@param status integer
---@param data any
---@return Response
local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data)
    }
end

-- Initialize users if not present
local function get_users()
    local users = state.get("users")
    if not users then
        users = {
            ["1"] = { id = "1", name = "Alice", email = "alice@example.com" },
            ["2"] = { id = "2", name = "Bob", email = "bob@example.com" }
        }
        state.set("users", users)
    end
    return users
end

---@param request Request
---@return Response
function handle(request)
    local users = get_users()

    -- GET /users - List all users
    if request.method == "GET" and request.path == "/users" then
        local list = {}
        for _, user in pairs(users) do
            table.insert(list, user)
        end
        return json_response(200, { users = list })
    end

    -- GET /users/:id - Get a specific user
    local user_id = request.path:match("^/users/([^/]+)$")
    if request.method == "GET" and user_id then
        local user = users[user_id]
        if user then
            return json_response(200, user)
        end
        return json_response(404, { error = "User not found" })
    end

    -- POST /users - Create a new user
    if request.method == "POST" and request.path == "/users" then
        local data = json.decode(request.body)
        local new_id = uuid.v4()
        local new_user = {
            id = new_id,
            name = data.name,
            email = data.email
        }
        users[new_id] = new_user
        state.set("users", users)
        return json_response(201, new_user)
    end

    -- PUT /users/:id - Update a user
    if request.method == "PUT" and user_id then
        local user = users[user_id]
        if not user then
            return json_response(404, { error = "User not found" })
        end
        local data = json.decode(request.body)
        user.name = data.name or user.name
        user.email = data.email or user.email
        users[user_id] = user
        state.set("users", users)
        return json_response(200, user)
    end

    -- DELETE /users/:id - Delete a user
    if request.method == "DELETE" and user_id then
        if not users[user_id] then
            return json_response(404, { error = "User not found" })
        end
        users[user_id] = nil
        state.set("users", users)
        return json_response(204, nil)
    end

    return json_response(404, { error = "Not found" })
end
```

### Offset-Based Pagination

Traditional pagination with `offset` and `limit` parameters.

```lua
-- api.example.com/init.lua

local json = require("json")

-- Generate sample data
local function generate_items(count)
    local items = {}
    for i = 1, count do
        table.insert(items, {
            id = i,
            name = "Item " .. i,
            created_at = "2024-01-" .. string.format("%02d", (i % 28) + 1)
        })
    end
    return items
end

local all_items = generate_items(100)

---@param request Request
---@return Response
function handle(request)
    if request.method == "GET" and request.path == "/items" then
        -- Parse pagination params with defaults
        local offset = tonumber(request.query.offset) or 0
        local limit = tonumber(request.query.limit) or 10

        -- Clamp limit to reasonable bounds
        if limit > 50 then limit = 50 end
        if limit < 1 then limit = 1 end

        -- Slice the items
        local items = {}
        for i = offset + 1, math.min(offset + limit, #all_items) do
            table.insert(items, all_items[i])
        end

        -- Calculate pagination metadata
        local total = #all_items
        local has_more = (offset + limit) < total

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                items = items,
                pagination = {
                    offset = offset,
                    limit = limit,
                    total = total,
                    has_more = has_more
                }
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Cursor-Based Pagination

Modern cursor pagination for infinite scroll or real-time data.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")
local time = require("time")

-- Initialize posts with timestamps
local function get_posts()
    local posts = state.get("posts")
    if not posts then
        posts = {}
        local base_time = time.now()
        for i = 1, 50 do
            table.insert(posts, {
                id = uuid.v4(),
                content = "Post #" .. i,
                created_at = base_time - (i * 3600),  -- 1 hour apart
                author = "user_" .. ((i % 5) + 1)
            })
        end
        state.set("posts", posts)
    end
    return posts
end

-- Encode cursor (timestamp + id for uniqueness)
local function encode_cursor(post)
    return post.created_at .. ":" .. post.id
end

-- Decode cursor
local function decode_cursor(cursor)
    local ts, id = cursor:match("^(%d+):(.+)$")
    return tonumber(ts), id
end

---@param request Request
---@return Response
function handle(request)
    if request.method == "GET" and request.path == "/posts" then
        local posts = get_posts()
        local limit = tonumber(request.query.limit) or 10
        local cursor = request.query.cursor

        if limit > 25 then limit = 25 end

        -- Find starting position
        local start_index = 1
        if cursor then
            local cursor_ts, cursor_id = decode_cursor(cursor)
            for i, post in ipairs(posts) do
                if post.created_at == cursor_ts and post.id == cursor_id then
                    start_index = i + 1
                    break
                elseif post.created_at < cursor_ts then
                    start_index = i
                    break
                end
            end
        end

        -- Get page of results
        local results = {}
        for i = start_index, math.min(start_index + limit - 1, #posts) do
            table.insert(results, posts[i])
        end

        -- Build next cursor
        local next_cursor = nil
        if #results == limit and (start_index + limit - 1) < #posts then
            next_cursor = encode_cursor(results[#results])
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                posts = results,
                next_cursor = next_cursor,
                has_more = next_cursor ~= nil
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Filtering and Search

Query parameter filtering with multiple criteria.

```lua
-- api.example.com/init.lua

local json = require("json")

local products = {
    { id = 1, name = "Laptop", category = "electronics", price = 999, in_stock = true },
    { id = 2, name = "Mouse", category = "electronics", price = 29, in_stock = true },
    { id = 3, name = "Desk", category = "furniture", price = 299, in_stock = false },
    { id = 4, name = "Chair", category = "furniture", price = 199, in_stock = true },
    { id = 5, name = "Monitor", category = "electronics", price = 399, in_stock = true },
    { id = 6, name = "Keyboard", category = "electronics", price = 79, in_stock = false },
    { id = 7, name = "Bookshelf", category = "furniture", price = 149, in_stock = true }
}

---@param request Request
---@return Response
function handle(request)
    if request.method == "GET" and request.path == "/products" then
        local results = {}

        for _, product in ipairs(products) do
            local matches = true

            -- Filter by category
            if request.query.category and product.category ~= request.query.category then
                matches = false
            end

            -- Filter by price range
            if request.query.min_price then
                local min = tonumber(request.query.min_price)
                if product.price < min then matches = false end
            end
            if request.query.max_price then
                local max = tonumber(request.query.max_price)
                if product.price > max then matches = false end
            end

            -- Filter by stock status
            if request.query.in_stock then
                local want_stock = request.query.in_stock == "true"
                if product.in_stock ~= want_stock then matches = false end
            end

            -- Search by name (case-insensitive substring)
            if request.query.search then
                local search = request.query.search:lower()
                if not product.name:lower():find(search, 1, true) then
                    matches = false
                end
            end

            if matches then
                table.insert(results, product)
            end
        end

        -- Sort by field
        local sort_by = request.query.sort or "id"
        local sort_order = request.query.order or "asc"

        table.sort(results, function(a, b)
            if sort_order == "desc" then
                return a[sort_by] > b[sort_by]
            else
                return a[sort_by] < b[sort_by]
            end
        end)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                products = results,
                count = #results,
                filters = {
                    category = request.query.category,
                    min_price = request.query.min_price,
                    max_price = request.query.max_price,
                    in_stock = request.query.in_stock,
                    search = request.query.search
                }
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Versioned APIs

Handle multiple API versions with different response formats.

```lua
-- api.example.com/init.lua

local json = require("json")

local users = {
    { id = 1, name = "Alice Smith", email = "alice@example.com", role = "admin" },
    { id = 2, name = "Bob Jones", email = "bob@example.com", role = "user" }
}

-- V1 response format: flat structure
local function format_user_v1(user)
    return {
        id = user.id,
        name = user.name,
        email = user.email
    }
end

-- V2 response format: nested structure with metadata
local function format_user_v2(user)
    return {
        data = {
            type = "user",
            id = tostring(user.id),
            attributes = {
                full_name = user.name,
                email_address = user.email,
                account_role = user.role
            }
        }
    }
end

---@param request Request
---@return Response
function handle(request)
    -- Detect API version from path prefix
    local version, path = request.path:match("^/(v%d+)(.*)$")

    if not version then
        return {
            status = 400,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "API version required",
                message = "Use /v1/ or /v2/ prefix"
            })
        }
    end

    -- GET /v1/users or /v2/users
    if request.method == "GET" and path == "/users" then
        local formatted = {}

        if version == "v1" then
            for _, user in ipairs(users) do
                table.insert(formatted, format_user_v1(user))
            end
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ users = formatted })
            }
        elseif version == "v2" then
            for _, user in ipairs(users) do
                table.insert(formatted, format_user_v2(user))
            end
            return {
                status = 200,
                headers = {
                    ["Content-Type"] = "application/vnd.api+json",
                    ["X-API-Version"] = "2.0"
                },
                body = json.encode({
                    data = formatted,
                    meta = { total = #formatted }
                })
            }
        else
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    error = "Unsupported API version",
                    supported = { "v1", "v2" }
                })
            }
        end
    end

    return {
        status = 404,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({ error = "Not found" })
    }
end
```

---

## Response Simulation

### Error Responses

Comprehensive error response handling for different HTTP status codes.

```lua
-- api.example.com/init.lua

local json = require("json")

-- Error response factory
local function error_response(status, code, message, details)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            error = {
                code = code,
                message = message,
                details = details
            }
        })
    }
end

---@param request Request
---@return Response
function handle(request)
    -- Simulate 400 Bad Request
    if request.path == "/error/bad-request" then
        return error_response(400, "INVALID_REQUEST", "The request body is malformed", {
            field = "email",
            issue = "Invalid email format"
        })
    end

    -- Simulate 401 Unauthorized
    if request.path == "/error/unauthorized" then
        return {
            status = 401,
            headers = {
                ["Content-Type"] = "application/json",
                ["WWW-Authenticate"] = 'Bearer realm="api"'
            },
            body = json.encode({
                error = {
                    code = "UNAUTHORIZED",
                    message = "Authentication required"
                }
            })
        }
    end

    -- Simulate 403 Forbidden
    if request.path == "/error/forbidden" then
        return error_response(403, "FORBIDDEN", "You do not have permission to access this resource", {
            required_role = "admin",
            your_role = "user"
        })
    end

    -- Simulate 404 Not Found
    if request.path == "/error/not-found" then
        return error_response(404, "NOT_FOUND", "The requested resource was not found", {
            resource_type = "user",
            resource_id = "12345"
        })
    end

    -- Simulate 409 Conflict
    if request.path == "/error/conflict" then
        return error_response(409, "CONFLICT", "A resource with this identifier already exists", {
            existing_id = "user_123",
            conflicting_field = "email"
        })
    end

    -- Simulate 422 Unprocessable Entity (validation errors)
    if request.path == "/error/validation" then
        return {
            status = 422,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = {
                    code = "VALIDATION_ERROR",
                    message = "Request validation failed",
                    errors = {
                        { field = "name", message = "Name is required" },
                        { field = "email", message = "Must be a valid email address" },
                        { field = "age", message = "Must be at least 18" }
                    }
                }
            })
        }
    end

    -- Simulate 429 Too Many Requests
    if request.path == "/error/rate-limit" then
        return {
            status = 429,
            headers = {
                ["Content-Type"] = "application/json",
                ["Retry-After"] = "60",
                ["X-RateLimit-Limit"] = "100",
                ["X-RateLimit-Remaining"] = "0",
                ["X-RateLimit-Reset"] = tostring(os.time() + 60)
            },
            body = json.encode({
                error = {
                    code = "RATE_LIMITED",
                    message = "Too many requests",
                    retry_after = 60
                }
            })
        }
    end

    -- Simulate 500 Internal Server Error
    if request.path == "/error/server-error" then
        return error_response(500, "INTERNAL_ERROR", "An unexpected error occurred", {
            request_id = "req_abc123",
            timestamp = os.date("!%Y-%m-%dT%H:%M:%SZ")
        })
    end

    -- Simulate 503 Service Unavailable
    if request.path == "/error/unavailable" then
        return {
            status = 503,
            headers = {
                ["Content-Type"] = "application/json",
                ["Retry-After"] = "300"
            },
            body = json.encode({
                error = {
                    code = "SERVICE_UNAVAILABLE",
                    message = "Service is temporarily unavailable for maintenance",
                    estimated_recovery = os.date("!%Y-%m-%dT%H:%M:%SZ", os.time() + 300)
                }
            })
        }
    end

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({ message = "OK" })
    }
end
```

### Slow Responses

Simulate network latency and slow backends using the delay module.

```lua
-- api.example.com/init.lua

local json = require("json")
local delay = require("delay")

---@param request Request
---@return Response
function handle(request)
    -- Fixed delay endpoint
    if request.path == "/slow/fixed" then
        delay.sleep(2000)  -- 2 second delay
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ message = "Response after 2 seconds" })
        }
    end

    -- Configurable delay via query param
    if request.path == "/slow/configurable" then
        local ms = tonumber(request.query.delay) or 1000
        -- Cap at 10 seconds to prevent abuse
        if ms > 10000 then ms = 10000 end

        delay.sleep(ms)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                message = "Response delivered",
                delay_ms = ms
            })
        }
    end

    -- Random delay to simulate variable latency
    if request.path == "/slow/random" then
        local min_ms = tonumber(request.query.min) or 100
        local max_ms = tonumber(request.query.max) or 2000
        local actual_delay = math.random(min_ms, max_ms)

        delay.sleep(actual_delay)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                message = "Variable latency response",
                actual_delay_ms = actual_delay
            })
        }
    end

    -- Simulate timeout (very slow response)
    if request.path == "/slow/timeout" then
        -- 30 second delay - useful for testing client timeouts
        delay.sleep(30000)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ message = "You waited!" })
        }
    end

    -- Simulate slow database query
    if request.path == "/slow/database" then
        -- Initial connection delay
        delay.sleep(50)

        -- Query execution delay
        delay.sleep(500)

        -- Result serialization delay
        delay.sleep(100)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                data = { id = 1, name = "Slow query result" },
                query_time_ms = 650
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Conditional Responses

Return different responses based on headers, body content, or other request attributes.

```lua
-- api.example.com/init.lua

local json = require("json")

---@param request Request
---@return Response
function handle(request)
    -- Response based on Accept header
    if request.path == "/content" then
        local accept = request.headers["accept"] or ""

        if accept:find("application/xml") then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/xml" },
                body = '<?xml version="1.0"?><response><message>XML response</message></response>'
            }
        elseif accept:find("text/plain") then
            return {
                status = 200,
                headers = { ["Content-Type"] = "text/plain" },
                body = "Plain text response"
            }
        else
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ message = "JSON response" })
            }
        end
    end

    -- Response based on User-Agent
    if request.path == "/client-info" then
        local ua = request.headers["user-agent"] or "unknown"

        local client_type = "unknown"
        if ua:find("Mobile") or ua:find("Android") or ua:find("iPhone") then
            client_type = "mobile"
        elseif ua:find("curl") then
            client_type = "cli"
        elseif ua:find("Mozilla") then
            client_type = "browser"
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                client_type = client_type,
                user_agent = ua,
                features = client_type == "mobile" and { "compact_view" } or { "full_view" }
            })
        }
    end

    -- Response based on request body content
    if request.method == "POST" and request.path == "/validate" then
        local data = json.decode(request.body)

        -- Validate email format (simple check)
        if data.email and not data.email:match("^[^@]+@[^@]+%.[^@]+$") then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    valid = false,
                    error = "Invalid email format"
                })
            }
        end

        -- Validate required fields
        if not data.name or data.name == "" then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    valid = false,
                    error = "Name is required"
                })
            }
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                valid = true,
                message = "Validation passed"
            })
        }
    end

    -- Response based on custom header
    if request.path == "/feature-flag" then
        local flags = request.headers["x-feature-flags"] or ""

        local features = {}
        for flag in flags:gmatch("[^,]+") do
            features[flag:match("^%s*(.-)%s*$")] = true  -- trim whitespace
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                beta_enabled = features["beta"] == true,
                dark_mode = features["dark-mode"] == true,
                new_dashboard = features["new-dashboard"] == true,
                raw_flags = flags
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

---

## Authentication Mocking

### Bearer Token Validation

Simulate JWT/Bearer token authentication.

```lua
-- api.example.com/init.lua

local json = require("json")

-- Simulated valid tokens (in real scenarios, you might decode JWTs)
local valid_tokens = {
    ["token_admin_123"] = { user_id = "1", role = "admin", name = "Admin User" },
    ["token_user_456"] = { user_id = "2", role = "user", name = "Regular User" },
    ["token_readonly_789"] = { user_id = "3", role = "readonly", name = "Read Only User" }
}

-- Extract bearer token from Authorization header
local function extract_token(request)
    local auth = request.headers["authorization"] or ""
    return auth:match("^Bearer%s+(.+)$")
end

-- Authentication middleware
local function authenticate(request)
    local token = extract_token(request)

    if not token then
        return nil, {
            status = 401,
            headers = {
                ["Content-Type"] = "application/json",
                ["WWW-Authenticate"] = 'Bearer realm="api"'
            },
            body = json.encode({
                error = "UNAUTHORIZED",
                message = "Bearer token required"
            })
        }
    end

    local user = valid_tokens[token]
    if not user then
        return nil, {
            status = 401,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "INVALID_TOKEN",
                message = "Token is invalid or expired"
            })
        }
    end

    return user, nil
end

-- Authorization check
local function require_role(user, required_role)
    local role_hierarchy = { readonly = 1, user = 2, admin = 3 }
    local user_level = role_hierarchy[user.role] or 0
    local required_level = role_hierarchy[required_role] or 0
    return user_level >= required_level
end

---@param request Request
---@return Response
function handle(request)
    -- Public endpoint (no auth required)
    if request.path == "/public" then
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ message = "Public endpoint" })
        }
    end

    -- Protected endpoint (any valid token)
    if request.path == "/me" then
        local user, err = authenticate(request)
        if err then return err end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                user_id = user.user_id,
                name = user.name,
                role = user.role
            })
        }
    end

    -- Admin-only endpoint
    if request.path == "/admin/users" then
        local user, err = authenticate(request)
        if err then return err end

        if not require_role(user, "admin") then
            return {
                status = 403,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    error = "FORBIDDEN",
                    message = "Admin role required"
                })
            }
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                users = {
                    { id = "1", name = "Admin User" },
                    { id = "2", name = "Regular User" }
                }
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### API Key Checking

Validate API keys via header or query parameter.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local time = require("time")

-- Initialize API keys
local function get_api_keys()
    local keys = state.get("api_keys")
    if not keys then
        keys = {
            ["sk_live_abc123"] = {
                name = "Production Key",
                tier = "premium",
                rate_limit = 1000,
                created_at = "2024-01-01"
            },
            ["sk_test_xyz789"] = {
                name = "Test Key",
                tier = "basic",
                rate_limit = 100,
                created_at = "2024-01-15"
            }
        }
        state.set("api_keys", keys)
    end
    return keys
end

-- Extract API key from request
local function extract_api_key(request)
    -- Check header first (preferred)
    local header_key = request.headers["x-api-key"]
    if header_key then return header_key end

    -- Fall back to query parameter
    return request.query.api_key
end

---@param request Request
---@return Response
function handle(request)
    local api_keys = get_api_keys()
    local api_key = extract_api_key(request)

    -- Validate API key presence
    if not api_key then
        return {
            status = 401,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "API_KEY_REQUIRED",
                message = "Provide API key via X-API-Key header or api_key query parameter"
            })
        }
    end

    -- Validate API key
    local key_info = api_keys[api_key]
    if not key_info then
        return {
            status = 401,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "INVALID_API_KEY",
                message = "The provided API key is invalid"
            })
        }
    end

    -- Check tier-based access
    if request.path == "/premium/data" and key_info.tier ~= "premium" then
        return {
            status = 403,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "UPGRADE_REQUIRED",
                message = "This endpoint requires a premium tier API key",
                current_tier = key_info.tier
            })
        }
    end

    -- Return data with rate limit headers
    return {
        status = 200,
        headers = {
            ["Content-Type"] = "application/json",
            ["X-RateLimit-Limit"] = tostring(key_info.rate_limit),
            ["X-RateLimit-Remaining"] = tostring(key_info.rate_limit - 1),
            ["X-API-Tier"] = key_info.tier
        },
        body = json.encode({
            message = "Authenticated successfully",
            key_name = key_info.name,
            tier = key_info.tier
        })
    }
end
```

### OAuth Token Exchange

Mock OAuth 2.0 authorization code and refresh token flows.

```lua
-- auth.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")
local time = require("time")

-- Initialize auth state
local function init_auth_state()
    if not state.get("auth_codes") then
        state.set("auth_codes", {})
    end
    if not state.get("refresh_tokens") then
        state.set("refresh_tokens", {})
    end
    if not state.get("access_tokens") then
        state.set("access_tokens", {})
    end
end

-- Generate tokens
local function generate_access_token()
    return "at_" .. uuid.v4():gsub("-", "")
end

local function generate_refresh_token()
    return "rt_" .. uuid.v4():gsub("-", "")
end

---@param request Request
---@return Response
function handle(request)
    init_auth_state()

    -- Authorization endpoint (returns auth code)
    if request.method == "GET" and request.path == "/authorize" then
        local client_id = request.query.client_id
        local redirect_uri = request.query.redirect_uri
        local response_type = request.query.response_type
        local scope = request.query.scope or "read"

        if response_type ~= "code" then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    error = "unsupported_response_type",
                    error_description = "Only 'code' response type is supported"
                })
            }
        end

        -- Generate authorization code
        local auth_code = "ac_" .. uuid.v4():gsub("-", "")
        local codes = state.get("auth_codes")
        codes[auth_code] = {
            client_id = client_id,
            redirect_uri = redirect_uri,
            scope = scope,
            created_at = time.now(),
            expires_at = time.now() + 600  -- 10 minutes
        }
        state.set("auth_codes", codes)

        -- In real OAuth, this would redirect. For mocking, return the code.
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                code = auth_code,
                redirect_uri = redirect_uri .. "?code=" .. auth_code
            })
        }
    end

    -- Token endpoint
    if request.method == "POST" and request.path == "/token" then
        local data = json.decode(request.body)
        local grant_type = data.grant_type

        -- Authorization code grant
        if grant_type == "authorization_code" then
            local codes = state.get("auth_codes")
            local code_info = codes[data.code]

            if not code_info then
                return {
                    status = 400,
                    headers = { ["Content-Type"] = "application/json" },
                    body = json.encode({
                        error = "invalid_grant",
                        error_description = "Authorization code is invalid or expired"
                    })
                }
            end

            -- Invalidate the code (single use)
            codes[data.code] = nil
            state.set("auth_codes", codes)

            -- Generate tokens
            local access_token = generate_access_token()
            local refresh_token = generate_refresh_token()

            -- Store tokens
            local access_tokens = state.get("access_tokens")
            access_tokens[access_token] = {
                scope = code_info.scope,
                expires_at = time.now() + 3600  -- 1 hour
            }
            state.set("access_tokens", access_tokens)

            local refresh_tokens = state.get("refresh_tokens")
            refresh_tokens[refresh_token] = {
                scope = code_info.scope,
                client_id = code_info.client_id
            }
            state.set("refresh_tokens", refresh_tokens)

            return {
                status = 200,
                headers = {
                    ["Content-Type"] = "application/json",
                    ["Cache-Control"] = "no-store"
                },
                body = json.encode({
                    access_token = access_token,
                    token_type = "Bearer",
                    expires_in = 3600,
                    refresh_token = refresh_token,
                    scope = code_info.scope
                })
            }
        end

        -- Refresh token grant
        if grant_type == "refresh_token" then
            local refresh_tokens = state.get("refresh_tokens")
            local token_info = refresh_tokens[data.refresh_token]

            if not token_info then
                return {
                    status = 400,
                    headers = { ["Content-Type"] = "application/json" },
                    body = json.encode({
                        error = "invalid_grant",
                        error_description = "Refresh token is invalid"
                    })
                }
            end

            -- Generate new access token
            local access_token = generate_access_token()
            local access_tokens = state.get("access_tokens")
            access_tokens[access_token] = {
                scope = token_info.scope,
                expires_at = time.now() + 3600
            }
            state.set("access_tokens", access_tokens)

            return {
                status = 200,
                headers = {
                    ["Content-Type"] = "application/json",
                    ["Cache-Control"] = "no-store"
                },
                body = json.encode({
                    access_token = access_token,
                    token_type = "Bearer",
                    expires_in = 3600,
                    scope = token_info.scope
                })
            }
        end

        return {
            status = 400,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                error = "unsupported_grant_type",
                error_description = "Grant type not supported"
            })
        }
    end

    -- Token introspection endpoint
    if request.method == "POST" and request.path == "/introspect" then
        local data = json.decode(request.body)
        local access_tokens = state.get("access_tokens")
        local token_info = access_tokens[data.token]

        if not token_info or token_info.expires_at < time.now() then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ active = false })
            }
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                active = true,
                scope = token_info.scope,
                exp = token_info.expires_at,
                token_type = "Bearer"
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

---

## Stateful Flows

### Shopping Cart

Complete shopping cart implementation with add, update, and checkout.

```lua
-- shop.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")
local time = require("time")

-- Product catalog
local products = {
    ["prod_001"] = { id = "prod_001", name = "Widget", price = 9.99 },
    ["prod_002"] = { id = "prod_002", name = "Gadget", price = 24.99 },
    ["prod_003"] = { id = "prod_003", name = "Gizmo", price = 14.99 }
}

-- Get or create cart
local function get_cart(cart_id)
    local carts = state.get("carts") or {}
    return carts[cart_id]
end

local function save_cart(cart_id, cart)
    local carts = state.get("carts") or {}
    carts[cart_id] = cart
    state.set("carts", carts)
end

local function calculate_total(cart)
    local total = 0
    for _, item in ipairs(cart.items) do
        total = total + (item.price * item.quantity)
    end
    return math.floor(total * 100) / 100  -- Round to 2 decimals
end

---@param request Request
---@return Response
function handle(request)
    -- Create new cart
    if request.method == "POST" and request.path == "/cart" then
        local cart_id = "cart_" .. uuid.v4()
        local cart = {
            id = cart_id,
            items = {},
            created_at = time.iso8601(),
            updated_at = time.iso8601()
        }
        save_cart(cart_id, cart)

        return {
            status = 201,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(cart)
        }
    end

    -- Get cart
    local cart_id = request.path:match("^/cart/([^/]+)$")
    if request.method == "GET" and cart_id then
        local cart = get_cart(cart_id)
        if not cart then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart not found" })
            }
        end

        cart.total = calculate_total(cart)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(cart)
        }
    end

    -- Add item to cart
    local cart_id_items = request.path:match("^/cart/([^/]+)/items$")
    if request.method == "POST" and cart_id_items then
        local cart = get_cart(cart_id_items)
        if not cart then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart not found" })
            }
        end

        local data = json.decode(request.body)
        local product = products[data.product_id]

        if not product then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Product not found" })
            }
        end

        -- Check if item already in cart
        local found = false
        for _, item in ipairs(cart.items) do
            if item.product_id == data.product_id then
                item.quantity = item.quantity + (data.quantity or 1)
                found = true
                break
            end
        end

        if not found then
            table.insert(cart.items, {
                product_id = data.product_id,
                name = product.name,
                price = product.price,
                quantity = data.quantity or 1
            })
        end

        cart.updated_at = time.iso8601()
        save_cart(cart_id_items, cart)

        cart.total = calculate_total(cart)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(cart)
        }
    end

    -- Update item quantity
    local cart_id_item, product_id = request.path:match("^/cart/([^/]+)/items/([^/]+)$")
    if request.method == "PUT" and cart_id_item and product_id then
        local cart = get_cart(cart_id_item)
        if not cart then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart not found" })
            }
        end

        local data = json.decode(request.body)
        local found = false

        for i, item in ipairs(cart.items) do
            if item.product_id == product_id then
                if data.quantity <= 0 then
                    table.remove(cart.items, i)
                else
                    item.quantity = data.quantity
                end
                found = true
                break
            end
        end

        if not found then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Item not in cart" })
            }
        end

        cart.updated_at = time.iso8601()
        save_cart(cart_id_item, cart)

        cart.total = calculate_total(cart)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(cart)
        }
    end

    -- Delete item from cart
    if request.method == "DELETE" and cart_id_item and product_id then
        local cart = get_cart(cart_id_item)
        if not cart then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart not found" })
            }
        end

        for i, item in ipairs(cart.items) do
            if item.product_id == product_id then
                table.remove(cart.items, i)
                break
            end
        end

        cart.updated_at = time.iso8601()
        save_cart(cart_id_item, cart)

        return { status = 204 }
    end

    -- Checkout
    local checkout_cart_id = request.path:match("^/cart/([^/]+)/checkout$")
    if request.method == "POST" and checkout_cart_id then
        local cart = get_cart(checkout_cart_id)
        if not cart then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart not found" })
            }
        end

        if #cart.items == 0 then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Cart is empty" })
            }
        end

        -- Create order
        local order = {
            id = "order_" .. uuid.v4(),
            items = cart.items,
            total = calculate_total(cart),
            status = "confirmed",
            created_at = time.iso8601()
        }

        -- Store order
        local orders = state.get("orders") or {}
        orders[order.id] = order
        state.set("orders", orders)

        -- Clear cart
        local carts = state.get("carts") or {}
        carts[checkout_cart_id] = nil
        state.set("carts", carts)

        return {
            status = 201,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(order)
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Multi-Step Wizard

Track progress through a multi-step form submission flow.

```lua
-- forms.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")
local time = require("time")

-- Define wizard steps and their validations
local wizard_steps = {
    { name = "personal_info", required_fields = { "first_name", "last_name", "email" } },
    { name = "address", required_fields = { "street", "city", "postal_code", "country" } },
    { name = "preferences", required_fields = { "newsletter" } },
    { name = "review", required_fields = {} }
}

local function get_session(session_id)
    local sessions = state.get("wizard_sessions") or {}
    return sessions[session_id]
end

local function save_session(session_id, session)
    local sessions = state.get("wizard_sessions") or {}
    sessions[session_id] = session
    state.set("wizard_sessions", sessions)
end

local function validate_step(step_index, data)
    local step = wizard_steps[step_index]
    local errors = {}

    for _, field in ipairs(step.required_fields) do
        if not data[field] or data[field] == "" then
            table.insert(errors, { field = field, message = field .. " is required" })
        end
    end

    -- Custom validations
    if step.name == "personal_info" and data.email then
        if not data.email:match("^[^@]+@[^@]+%.[^@]+$") then
            table.insert(errors, { field = "email", message = "Invalid email format" })
        end
    end

    return errors
end

---@param request Request
---@return Response
function handle(request)
    -- Start new wizard session
    if request.method == "POST" and request.path == "/wizard/start" then
        local session_id = "wiz_" .. uuid.v4()
        local session = {
            id = session_id,
            current_step = 1,
            data = {},
            completed_steps = {},
            created_at = time.iso8601(),
            updated_at = time.iso8601()
        }
        save_session(session_id, session)

        return {
            status = 201,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                session_id = session_id,
                current_step = wizard_steps[1].name,
                total_steps = #wizard_steps,
                required_fields = wizard_steps[1].required_fields
            })
        }
    end

    -- Get session status
    local session_id = request.path:match("^/wizard/([^/]+)$")
    if request.method == "GET" and session_id then
        local session = get_session(session_id)
        if not session then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Session not found" })
            }
        end

        local current = wizard_steps[session.current_step]
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                session_id = session.id,
                current_step = session.current_step,
                current_step_name = current.name,
                total_steps = #wizard_steps,
                completed_steps = session.completed_steps,
                data = session.data
            })
        }
    end

    -- Submit current step
    local submit_session_id = request.path:match("^/wizard/([^/]+)/submit$")
    if request.method == "POST" and submit_session_id then
        local session = get_session(submit_session_id)
        if not session then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Session not found" })
            }
        end

        if session.current_step > #wizard_steps then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Wizard already completed" })
            }
        end

        local data = json.decode(request.body)
        local errors = validate_step(session.current_step, data)

        if #errors > 0 then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    error = "Validation failed",
                    errors = errors
                })
            }
        end

        -- Merge step data
        local step_name = wizard_steps[session.current_step].name
        session.data[step_name] = data
        table.insert(session.completed_steps, step_name)

        -- Advance to next step
        session.current_step = session.current_step + 1
        session.updated_at = time.iso8601()
        save_session(submit_session_id, session)

        -- Check if completed
        if session.current_step > #wizard_steps then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    status = "completed",
                    message = "Wizard completed successfully",
                    data = session.data
                })
            }
        end

        local next_step = wizard_steps[session.current_step]
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                status = "in_progress",
                current_step = session.current_step,
                current_step_name = next_step.name,
                required_fields = next_step.required_fields,
                completed_steps = session.completed_steps
            })
        }
    end

    -- Go back to previous step
    local back_session_id = request.path:match("^/wizard/([^/]+)/back$")
    if request.method == "POST" and back_session_id then
        local session = get_session(back_session_id)
        if not session then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Session not found" })
            }
        end

        if session.current_step <= 1 then
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Already at first step" })
            }
        end

        session.current_step = session.current_step - 1
        table.remove(session.completed_steps)
        session.updated_at = time.iso8601()
        save_session(back_session_id, session)

        local current = wizard_steps[session.current_step]
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                current_step = session.current_step,
                current_step_name = current.name,
                required_fields = current.required_fields,
                previous_data = session.data[current.name]
            })
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Rate Limiting Simulation

Implement sliding window rate limiting with headers.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local time = require("time")

-- Rate limit configuration
local RATE_LIMIT = 10           -- requests per window
local WINDOW_SIZE = 60          -- seconds

-- Get client identifier (from API key or IP simulation)
local function get_client_id(request)
    return request.headers["x-api-key"] or request.headers["x-forwarded-for"] or "anonymous"
end

-- Check and update rate limit
local function check_rate_limit(client_id)
    local now = time.now()
    local window_start = now - WINDOW_SIZE

    -- Get client's request history
    local rate_limits = state.get("rate_limits") or {}
    local client_data = rate_limits[client_id] or { requests = {} }

    -- Clean old requests outside window
    local recent_requests = {}
    for _, ts in ipairs(client_data.requests) do
        if ts > window_start then
            table.insert(recent_requests, ts)
        end
    end

    -- Check if over limit
    local request_count = #recent_requests
    local remaining = RATE_LIMIT - request_count
    local reset_time = now + WINDOW_SIZE

    if remaining <= 0 then
        -- Find oldest request to calculate retry time
        local oldest = recent_requests[1] or now
        local retry_after = (oldest + WINDOW_SIZE) - now

        return {
            allowed = false,
            limit = RATE_LIMIT,
            remaining = 0,
            reset = reset_time,
            retry_after = math.max(1, math.ceil(retry_after))
        }
    end

    -- Record this request
    table.insert(recent_requests, now)
    client_data.requests = recent_requests
    rate_limits[client_id] = client_data
    state.set("rate_limits", rate_limits)

    return {
        allowed = true,
        limit = RATE_LIMIT,
        remaining = remaining - 1,  -- After this request
        reset = reset_time
    }
end

---@param request Request
---@return Response
function handle(request)
    local client_id = get_client_id(request)
    local rate_check = check_rate_limit(client_id)

    -- Common rate limit headers
    local rate_headers = {
        ["X-RateLimit-Limit"] = tostring(rate_check.limit),
        ["X-RateLimit-Remaining"] = tostring(rate_check.remaining),
        ["X-RateLimit-Reset"] = tostring(rate_check.reset)
    }

    if not rate_check.allowed then
        rate_headers["Retry-After"] = tostring(rate_check.retry_after)
        rate_headers["Content-Type"] = "application/json"

        return {
            status = 429,
            headers = rate_headers,
            body = json.encode({
                error = "RATE_LIMIT_EXCEEDED",
                message = "Too many requests",
                retry_after = rate_check.retry_after
            })
        }
    end

    -- Process normal request
    if request.path == "/data" then
        rate_headers["Content-Type"] = "application/json"
        return {
            status = 200,
            headers = rate_headers,
            body = json.encode({
                message = "Success",
                data = { value = math.random(1, 100) }
            })
        }
    end

    rate_headers["Content-Type"] = "application/json"
    return {
        status = 404,
        headers = rate_headers,
        body = json.encode({ error = "Not found" })
    }
end
```

### Request Counting

Track request counts for analytics or testing verification.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local time = require("time")

-- Initialize counters
local function get_counters()
    local counters = state.get("request_counters")
    if not counters then
        counters = {
            total = 0,
            by_method = {},
            by_path = {},
            by_status = {},
            history = {}
        }
        state.set("request_counters", counters)
    end
    return counters
end

-- Record a request
local function record_request(method, path, status)
    local counters = get_counters()

    -- Increment total
    counters.total = counters.total + 1

    -- By method
    counters.by_method[method] = (counters.by_method[method] or 0) + 1

    -- By path (normalize dynamic segments)
    local normalized_path = path:gsub("/%d+", "/:id")
    counters.by_path[normalized_path] = (counters.by_path[normalized_path] or 0) + 1

    -- By status
    local status_str = tostring(status)
    counters.by_status[status_str] = (counters.by_status[status_str] or 0) + 1

    -- Add to history (keep last 100)
    table.insert(counters.history, 1, {
        method = method,
        path = path,
        status = status,
        timestamp = time.iso8601()
    })
    if #counters.history > 100 then
        table.remove(counters.history)
    end

    state.set("request_counters", counters)
end

---@param request Request
---@return Response
function handle(request)
    -- Get stats endpoint (don't count this one)
    if request.path == "/_stats" then
        local counters = get_counters()
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(counters)
        }
    end

    -- Reset stats endpoint
    if request.method == "DELETE" and request.path == "/_stats" then
        state.delete("request_counters")
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ message = "Counters reset" })
        }
    end

    -- Get recent requests
    if request.path == "/_stats/history" then
        local counters = get_counters()
        local limit = tonumber(request.query.limit) or 10
        local history = {}
        for i = 1, math.min(limit, #counters.history) do
            table.insert(history, counters.history[i])
        end
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ requests = history })
        }
    end

    -- Sample API endpoints to count
    local response
    if request.path == "/users" then
        response = {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ users = {} })
        }
    elseif request.path:match("^/users/%d+$") then
        response = {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ id = 1, name = "User" })
        }
    else
        response = {
            status = 404,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ error = "Not found" })
        }
    end

    -- Record this request
    record_request(request.method, request.path, response.status)

    return response
end
```

---

## GraphQL Patterns

### Query Handling

Handle GraphQL queries with variable support.

```lua
-- graphql.example.com/init.lua

local json = require("json")

-- Sample data
local users = {
    { id = "1", name = "Alice", email = "alice@example.com", role = "admin" },
    { id = "2", name = "Bob", email = "bob@example.com", role = "user" },
    { id = "3", name = "Charlie", email = "charlie@example.com", role = "user" }
}

local posts = {
    { id = "1", title = "First Post", content = "Hello world", author_id = "1" },
    { id = "2", title = "Second Post", content = "GraphQL is great", author_id = "2" },
    { id = "3", title = "Third Post", content = "Lua scripting", author_id = "1" }
}

-- Helper to find by ID
local function find_by_id(collection, id)
    for _, item in ipairs(collection) do
        if item.id == id then
            return item
        end
    end
    return nil
end

-- Helper to filter collection
local function filter_by(collection, field, value)
    local results = {}
    for _, item in ipairs(collection) do
        if item[field] == value then
            table.insert(results, item)
        end
    end
    return results
end

---@param request Request
---@return Response
function handle(request)
    if request.method ~= "POST" or request.path ~= "/graphql" then
        return { status = 404, body = "Not found" }
    end

    local body = json.decode(request.body)
    local query = body.query or ""
    local variables = body.variables or {}

    -- Query: users (with optional role filter)
    if query:match("{%s*users") then
        local result = users
        if variables.role then
            result = filter_by(users, "role", variables.role)
        end
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = { users = result } })
        }
    end

    -- Query: user(id: ID!)
    if query:match("{%s*user") then
        local user = find_by_id(users, variables.id)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = { user = user } })
        }
    end

    -- Query: posts (with optional author filter)
    if query:match("{%s*posts") then
        local result = posts
        if variables.authorId then
            result = filter_by(posts, "author_id", variables.authorId)
        end

        -- Resolve author for each post if requested
        if query:match("author%s*{") then
            for _, post in ipairs(result) do
                post.author = find_by_id(users, post.author_id)
            end
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = { posts = result } })
        }
    end

    -- Query: post(id: ID!)
    if query:match("{%s*post") then
        local post = find_by_id(posts, variables.id)
        if post and query:match("author%s*{") then
            post.author = find_by_id(users, post.author_id)
        end
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = { post = post } })
        }
    end

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            errors = {{ message = "Unknown query", locations = {}, path = {} }}
        })
    }
end
```

### Mutation Responses

Handle GraphQL mutations with input validation.

```lua
-- graphql.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")

-- Initialize data
local function get_users()
    local users = state.get("users")
    if not users then
        users = {
            { id = "1", name = "Alice", email = "alice@example.com" }
        }
        state.set("users", users)
    end
    return users
end

local function save_users(users)
    state.set("users", users)
end

-- Validate email format
local function validate_email(email)
    return email and email:match("^[^@]+@[^@]+%.[^@]+$")
end

---@param request Request
---@return Response
function handle(request)
    if request.method ~= "POST" or request.path ~= "/graphql" then
        return { status = 404, body = "Not found" }
    end

    local body = json.decode(request.body)
    local query = body.query or ""
    local variables = body.variables or {}
    local users = get_users()

    -- Mutation: createUser
    if query:match("mutation") and query:match("createUser") then
        local input = variables.input or {}

        -- Validate input
        local errors = {}
        if not input.name or input.name == "" then
            table.insert(errors, {
                message = "Name is required",
                path = { "input", "name" }
            })
        end
        if not validate_email(input.email) then
            table.insert(errors, {
                message = "Valid email is required",
                path = { "input", "email" }
            })
        end

        -- Check for duplicate email
        for _, user in ipairs(users) do
            if user.email == input.email then
                table.insert(errors, {
                    message = "Email already exists",
                    path = { "input", "email" }
                })
                break
            end
        end

        if #errors > 0 then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    data = { createUser = nil },
                    errors = errors
                })
            }
        end

        local new_user = {
            id = uuid.v4(),
            name = input.name,
            email = input.email
        }
        table.insert(users, new_user)
        save_users(users)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                data = { createUser = { user = new_user } }
            })
        }
    end

    -- Mutation: updateUser
    if query:match("mutation") and query:match("updateUser") then
        local id = variables.id
        local input = variables.input or {}

        local user_index = nil
        for i, user in ipairs(users) do
            if user.id == id then
                user_index = i
                break
            end
        end

        if not user_index then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    data = { updateUser = nil },
                    errors = {{ message = "User not found", path = { "id" } }}
                })
            }
        end

        local user = users[user_index]
        if input.name then user.name = input.name end
        if input.email then
            if not validate_email(input.email) then
                return {
                    status = 200,
                    headers = { ["Content-Type"] = "application/json" },
                    body = json.encode({
                        data = { updateUser = nil },
                        errors = {{ message = "Invalid email format", path = { "input", "email" } }}
                    })
                }
            end
            user.email = input.email
        end

        users[user_index] = user
        save_users(users)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                data = { updateUser = { user = user } }
            })
        }
    end

    -- Mutation: deleteUser
    if query:match("mutation") and query:match("deleteUser") then
        local id = variables.id
        local deleted = false

        for i, user in ipairs(users) do
            if user.id == id then
                table.remove(users, i)
                deleted = true
                break
            end
        end

        save_users(users)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                data = { deleteUser = { success = deleted } }
            })
        }
    end

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            errors = {{ message = "Unknown operation" }}
        })
    }
end
```

### Resolver-Style Routing

Organize GraphQL handlers using a resolver pattern.

```lua
-- graphql.example.com/init.lua

local json = require("json")
local state = require("state")
local uuid = require("uuid")

-- Data access layer
local db = {
    users = {
        { id = "1", name = "Alice", team_id = "team_a" },
        { id = "2", name = "Bob", team_id = "team_a" },
        { id = "3", name = "Charlie", team_id = "team_b" }
    },
    teams = {
        { id = "team_a", name = "Alpha Team" },
        { id = "team_b", name = "Beta Team" }
    },
    tasks = {
        { id = "t1", title = "Task 1", assignee_id = "1", status = "done" },
        { id = "t2", title = "Task 2", assignee_id = "2", status = "in_progress" },
        { id = "t3", title = "Task 3", assignee_id = "1", status = "todo" }
    }
}

-- Query resolvers
local Query = {
    users = function(args)
        if args.teamId then
            local result = {}
            for _, u in ipairs(db.users) do
                if u.team_id == args.teamId then
                    table.insert(result, u)
                end
            end
            return result
        end
        return db.users
    end,

    user = function(args)
        for _, u in ipairs(db.users) do
            if u.id == args.id then return u end
        end
        return nil
    end,

    teams = function()
        return db.teams
    end,

    team = function(args)
        for _, t in ipairs(db.teams) do
            if t.id == args.id then return t end
        end
        return nil
    end,

    tasks = function(args)
        local result = db.tasks
        if args.status then
            local filtered = {}
            for _, t in ipairs(result) do
                if t.status == args.status then
                    table.insert(filtered, t)
                end
            end
            result = filtered
        end
        return result
    end
}

-- Type resolvers (for nested fields)
local User = {
    team = function(user)
        for _, t in ipairs(db.teams) do
            if t.id == user.team_id then return t end
        end
        return nil
    end,

    tasks = function(user)
        local result = {}
        for _, t in ipairs(db.tasks) do
            if t.assignee_id == user.id then
                table.insert(result, t)
            end
        end
        return result
    end
}

local Team = {
    members = function(team)
        local result = {}
        for _, u in ipairs(db.users) do
            if u.team_id == team.id then
                table.insert(result, u)
            end
        end
        return result
    end
}

local Task = {
    assignee = function(task)
        for _, u in ipairs(db.users) do
            if u.id == task.assignee_id then return u end
        end
        return nil
    end
}

-- Mutation resolvers
local Mutation = {
    createTask = function(args)
        local task = {
            id = "t" .. uuid.v4():sub(1, 8),
            title = args.input.title,
            assignee_id = args.input.assigneeId,
            status = "todo"
        }
        table.insert(db.tasks, task)
        return { task = task }
    end,

    updateTaskStatus = function(args)
        for i, t in ipairs(db.tasks) do
            if t.id == args.id then
                db.tasks[i].status = args.status
                return { task = db.tasks[i] }
            end
        end
        return { task = nil }
    end
}

-- Simple query parser (extracts operation and field names)
local function parse_query(query)
    local operation = query:match("^%s*(%w+)") or "query"
    local fields = {}

    for field in query:gmatch("{%s*(%w+)") do
        table.insert(fields, field)
    end

    return operation, fields
end

-- Resolve nested fields
local function resolve_nested(data, query, resolvers)
    if type(data) ~= "table" then return data end

    if data[1] then  -- Array
        for i, item in ipairs(data) do
            data[i] = resolve_nested(item, query, resolvers)
        end
    else  -- Object
        for field, resolver in pairs(resolvers) do
            if query:match(field .. "%s*{") then
                data[field] = resolver(data)
                -- Recursively resolve
                if field == "team" then
                    data[field] = resolve_nested(data[field], query, Team)
                elseif field == "members" or field == "assignee" then
                    data[field] = resolve_nested(data[field], query, User)
                elseif field == "tasks" then
                    data[field] = resolve_nested(data[field], query, Task)
                end
            end
        end
    end

    return data
end

---@param request Request
---@return Response
function handle(request)
    if request.method ~= "POST" or request.path ~= "/graphql" then
        return { status = 404, body = "Not found" }
    end

    local body = json.decode(request.body)
    local query = body.query or ""
    local variables = body.variables or {}

    local operation, fields = parse_query(query)

    -- Handle queries
    if operation == "query" then
        local data = {}

        for _, field in ipairs(fields) do
            if Query[field] then
                local result = Query[field](variables)

                -- Resolve nested fields based on query
                if field == "users" or field == "user" then
                    result = resolve_nested(result, query, User)
                elseif field == "teams" or field == "team" then
                    result = resolve_nested(result, query, Team)
                elseif field == "tasks" then
                    result = resolve_nested(result, query, Task)
                end

                data[field] = result
            end
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = data })
        }
    end

    -- Handle mutations
    if operation == "mutation" then
        local data = {}

        for _, field in ipairs(fields) do
            if Mutation[field] then
                data[field] = Mutation[field](variables)
            end
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ data = data })
        }
    end

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({
            errors = {{ message = "Unsupported operation: " .. operation }}
        })
    }
end
```

---

## Advanced Patterns

### Reading Fixture Files

Load test data from JSON files using the fs module.

```lua
-- api.example.com/init.lua

local json = require("json")
local fs = require("fs")
local log = require("log")

-- Cache loaded fixtures
local fixture_cache = {}

-- Load fixture with caching
local function load_fixture(path)
    if fixture_cache[path] then
        return fixture_cache[path]
    end

    if not fs.exists(path) then
        log.warn("Fixture not found: " .. path)
        return nil
    end

    local content = fs.read(path)
    local data = json.decode(content)
    fixture_cache[path] = data

    log.info("Loaded fixture: " .. path)
    return data
end

-- Load fixture or return default
local function load_fixture_or_default(path, default)
    local data = load_fixture(path)
    return data or default
end

---@param request Request
---@return Response
function handle(request)
    -- Serve users from fixture file
    if request.method == "GET" and request.path == "/users" then
        local users = load_fixture_or_default("fixtures/users.json", { users = {} })
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(users)
        }
    end

    -- Serve products from fixture file
    if request.method == "GET" and request.path == "/products" then
        local products = load_fixture_or_default("fixtures/products.json", { products = {} })
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(products)
        }
    end

    -- Dynamic fixture loading based on path
    -- GET /fixtures/users -> loads fixtures/users.json
    local fixture_name = request.path:match("^/fixtures/([%w_-]+)$")
    if request.method == "GET" and fixture_name then
        local path = "fixtures/" .. fixture_name .. ".json"

        if not fs.exists(path) then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({
                    error = "Fixture not found",
                    fixture = fixture_name
                })
            }
        end

        local data = load_fixture(path)
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(data)
        }
    end

    -- List available fixtures
    if request.method == "GET" and request.path == "/fixtures" then
        -- Check for known fixtures
        local available = {}
        local known_fixtures = { "users", "products", "orders", "config" }

        for _, name in ipairs(known_fixtures) do
            local path = "fixtures/" .. name .. ".json"
            if fs.exists(path) then
                table.insert(available, name)
            end
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ fixtures = available })
        }
    end

    return { status = 404, body = "Not found" }
end
```

Example fixture file (`fixtures/users.json`):

```json
{
  "users": [
    { "id": 1, "name": "Alice", "email": "alice@example.com" },
    { "id": 2, "name": "Bob", "email": "bob@example.com" }
  ]
}
```

### Sharing State Across Requests

Use the state module for cross-request data sharing and coordination.

```lua
-- api.example.com/init.lua

local json = require("json")
local state = require("state")
local time = require("time")
local uuid = require("uuid")

-- Feature flags that can be toggled at runtime
local function get_feature_flags()
    local flags = state.get("feature_flags")
    if not flags then
        flags = {
            new_checkout = false,
            dark_mode = true,
            beta_features = false
        }
        state.set("feature_flags", flags)
    end
    return flags
end

-- Session management across requests
local function get_sessions()
    return state.get("sessions") or {}
end

local function save_sessions(sessions)
    state.set("sessions", sessions)
end

-- A/B test assignment (sticky per user)
local function get_ab_assignment(user_id, experiment)
    local assignments = state.get("ab_assignments") or {}
    local key = experiment .. ":" .. user_id

    if not assignments[key] then
        -- Randomly assign to variant (would use consistent hashing in production)
        assignments[key] = math.random() < 0.5 and "control" and "treatment"
        state.set("ab_assignments", assignments)
    end

    return assignments[key]
end

---@param request Request
---@return Response
function handle(request)
    -- Feature flags endpoints
    if request.path == "/flags" then
        if request.method == "GET" then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode(get_feature_flags())
            }
        end

        if request.method == "PUT" then
            local data = json.decode(request.body)
            local flags = get_feature_flags()
            for key, value in pairs(data) do
                if flags[key] ~= nil then
                    flags[key] = value
                end
            end
            state.set("feature_flags", flags)
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode(flags)
            }
        end
    end

    -- Session management
    if request.method == "POST" and request.path == "/sessions" then
        local data = json.decode(request.body)
        local session_id = "sess_" .. uuid.v4()
        local sessions = get_sessions()

        sessions[session_id] = {
            user_id = data.user_id,
            created_at = time.iso8601(),
            last_activity = time.iso8601(),
            data = {}
        }
        save_sessions(sessions)

        return {
            status = 201,
            headers = {
                ["Content-Type"] = "application/json",
                ["Set-Cookie"] = "session_id=" .. session_id .. "; HttpOnly; Path=/"
            },
            body = json.encode({ session_id = session_id })
        }
    end

    -- Get session by ID
    local session_id = request.path:match("^/sessions/([^/]+)$")
    if request.method == "GET" and session_id then
        local sessions = get_sessions()
        local session = sessions[session_id]

        if not session then
            return {
                status = 404,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Session not found" })
            }
        end

        -- Update last activity
        session.last_activity = time.iso8601()
        sessions[session_id] = session
        save_sessions(sessions)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(session)
        }
    end

    -- A/B test endpoint
    if request.method == "GET" and request.path == "/experiment" then
        local user_id = request.query.user_id or "anonymous"
        local experiment = request.query.experiment or "default_experiment"

        local variant = get_ab_assignment(user_id, experiment)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({
                experiment = experiment,
                variant = variant,
                user_id = user_id
            })
        }
    end

    -- Global counter (demonstrates atomic-like operations)
    if request.path == "/counter" then
        local counter = state.get("global_counter") or 0

        if request.method == "GET" then
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ count = counter })
            }
        end

        if request.method == "POST" then
            counter = counter + 1
            state.set("global_counter", counter)
            return {
                status = 200,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ count = counter })
            }
        end
    end

    return { status = 404, body = "Not found" }
end
```

### Dynamic Response Generation

Generate responses dynamically based on request parameters and templates.

```lua
-- api.example.com/init.lua

local json = require("json")
local uuid = require("uuid")
local time = require("time")

-- Generate random data
local function random_string(length)
    local chars = "abcdefghijklmnopqrstuvwxyz"
    local result = {}
    for i = 1, length do
        local idx = math.random(1, #chars)
        table.insert(result, chars:sub(idx, idx))
    end
    return table.concat(result)
end

local function random_email()
    return random_string(8) .. "@example.com"
end

local function random_phone()
    return string.format("+1-%03d-%03d-%04d",
        math.random(100, 999),
        math.random(100, 999),
        math.random(1000, 9999)
    )
end

-- Generate fake user
local function generate_user()
    local first_names = { "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank" }
    local last_names = { "Smith", "Johnson", "Williams", "Brown", "Jones", "Davis" }

    return {
        id = uuid.v4(),
        first_name = first_names[math.random(1, #first_names)],
        last_name = last_names[math.random(1, #last_names)],
        email = random_email(),
        phone = random_phone(),
        created_at = time.iso8601()
    }
end

-- Generate fake product
local function generate_product()
    local adjectives = { "Amazing", "Premium", "Deluxe", "Super", "Ultra", "Pro" }
    local nouns = { "Widget", "Gadget", "Device", "Tool", "Kit", "System" }

    return {
        id = "prod_" .. uuid.v4():sub(1, 8),
        name = adjectives[math.random(1, #adjectives)] .. " " .. nouns[math.random(1, #nouns)],
        price = math.floor(math.random(999, 99999)) / 100,
        in_stock = math.random() > 0.2,
        sku = "SKU-" .. string.upper(random_string(6))
    }
end

-- Generate fake order
local function generate_order()
    local items = {}
    local item_count = math.random(1, 5)
    local total = 0

    for i = 1, item_count do
        local qty = math.random(1, 3)
        local price = math.floor(math.random(500, 5000)) / 100
        table.insert(items, {
            product_id = "prod_" .. uuid.v4():sub(1, 8),
            quantity = qty,
            unit_price = price
        })
        total = total + (qty * price)
    end

    local statuses = { "pending", "processing", "shipped", "delivered" }

    return {
        id = "order_" .. uuid.v4():sub(1, 8),
        items = items,
        total = math.floor(total * 100) / 100,
        status = statuses[math.random(1, #statuses)],
        created_at = time.iso8601()
    }
end

---@param request Request
---@return Response
function handle(request)
    -- Generate multiple users
    if request.method == "GET" and request.path == "/generate/users" then
        local count = tonumber(request.query.count) or 10
        if count > 100 then count = 100 end

        local users = {}
        for i = 1, count do
            table.insert(users, generate_user())
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ users = users, count = count })
        }
    end

    -- Generate single user
    if request.method == "GET" and request.path == "/generate/user" then
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(generate_user())
        }
    end

    -- Generate products
    if request.method == "GET" and request.path == "/generate/products" then
        local count = tonumber(request.query.count) or 10
        if count > 100 then count = 100 end

        local products = {}
        for i = 1, count do
            table.insert(products, generate_product())
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ products = products })
        }
    end

    -- Generate order
    if request.method == "GET" and request.path == "/generate/order" then
        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(generate_order())
        }
    end

    -- Echo endpoint with dynamic fields
    if request.method == "POST" and request.path == "/echo" then
        local data = json.decode(request.body)

        -- Add server-generated fields
        data._id = uuid.v4()
        data._timestamp = time.iso8601()
        data._request_id = "req_" .. uuid.v4():sub(1, 8)

        return {
            status = 200,
            headers = {
                ["Content-Type"] = "application/json",
                ["X-Request-Id"] = data._request_id
            },
            body = json.encode(data)
        }
    end

    -- Template response with variable substitution
    if request.method == "POST" and request.path == "/template" then
        local data = json.decode(request.body)
        local template = data.template or {}

        -- Substitute special values
        local function substitute(obj)
            if type(obj) == "string" then
                if obj == "{{uuid}}" then return uuid.v4() end
                if obj == "{{timestamp}}" then return time.iso8601() end
                if obj == "{{now}}" then return time.now() end
                if obj == "{{random_email}}" then return random_email() end
                return obj
            elseif type(obj) == "table" then
                local result = {}
                for k, v in pairs(obj) do
                    result[k] = substitute(v)
                end
                return result
            end
            return obj
        end

        local result = substitute(template)

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode(result)
        }
    end

    return { status = 404, body = "Not found" }
end
```

### Logging for Debugging

Use the log module effectively for debugging and monitoring.

```lua
-- api.example.com/init.lua

local json = require("json")
local log = require("log")
local time = require("time")

-- Log request details
local function log_request(request)
    log.info(string.format("[%s] %s %s",
        request.domain,
        request.method,
        request.path
    ))

    if request.query and next(request.query) then
        log.debug("Query params: " .. json.encode(request.query))
    end

    if request.body and request.body ~= "" then
        -- Truncate long bodies in debug log
        local body_preview = request.body
        if #body_preview > 200 then
            body_preview = body_preview:sub(1, 200) .. "... (truncated)"
        end
        log.debug("Request body: " .. body_preview)
    end

    -- Log specific headers of interest
    local auth = request.headers["authorization"]
    if auth then
        -- Don't log full token, just type
        local auth_type = auth:match("^(%w+)") or "unknown"
        log.debug("Auth type: " .. auth_type)
    end
end

-- Log response details
local function log_response(response, start_time)
    local duration = time.now_ms() - start_time
    log.info(string.format("Response: %d (%dms)", response.status, duration))

    if response.status >= 400 then
        log.warn("Error response: " .. (response.body or "no body"))
    end
end

-- Wrap handler with logging
local function with_logging(handler)
    return function(request)
        local start_time = time.now_ms()

        log_request(request)

        -- Call actual handler
        local ok, response = pcall(handler, request)

        if not ok then
            -- Handler threw an error
            log.error("Handler error: " .. tostring(response))
            response = {
                status = 500,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Internal server error" })
            }
        end

        log_response(response, start_time)

        return response
    end
end

-- Actual request handler
local function handler(request)
    -- Log custom events
    if request.path == "/users" and request.method == "POST" then
        local data = json.decode(request.body)
        log.info("Creating new user: " .. (data.email or "unknown"))

        -- Simulate validation
        if not data.email then
            log.warn("User creation failed: missing email")
            return {
                status = 400,
                headers = { ["Content-Type"] = "application/json" },
                body = json.encode({ error = "Email required" })
            }
        end

        log.info("User created successfully")
        return {
            status = 201,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ id = "user_123", email = data.email })
        }
    end

    -- Debug endpoint to test log levels
    if request.path == "/debug/logs" then
        log.debug("This is a debug message")
        log.info("This is an info message")
        log.warn("This is a warning message")
        log.error("This is an error message")

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ message = "Check server logs" })
        }
    end

    -- Conditional debug logging
    if request.path == "/debug/verbose" then
        local verbose = request.query.verbose == "true"

        if verbose then
            log.debug("Verbose mode enabled")
            log.debug("Full headers: " .. json.encode(request.headers))
            log.debug("Full query: " .. json.encode(request.query))
        end

        return {
            status = 200,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ verbose = verbose })
        }
    end

    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({ message = "OK" })
    }
end

-- Export wrapped handler
handle = with_logging(handler)
```

---

## Module Reference

Quick reference for all available modules:

| Module | Function | Description |
|--------|----------|-------------|
| `json` | `encode(value)` | Convert Lua value to JSON string |
| `json` | `decode(string)` | Parse JSON string to Lua value |
| `delay` | `sleep(ms)` | Async sleep for specified milliseconds |
| `log` | `debug(msg)` | Log debug message (requires `-v` flag) |
| `log` | `info(msg)` | Log info message |
| `log` | `warn(msg)` | Log warning message |
| `log` | `error(msg)` | Log error message |
| `state` | `get(key)` | Get value by key (returns nil if not found) |
| `state` | `set(key, value)` | Store value by key |
| `state` | `delete(key)` | Remove value by key |
| `state` | `clear()` | Remove all stored values |
| `uuid` | `v4()` | Generate random UUID v4 string |
| `time` | `now()` | Current Unix timestamp (seconds) |
| `time` | `now_ms()` | Current Unix timestamp (milliseconds) |
| `time` | `iso8601()` | Current time as ISO 8601 string |
| `time` | `format(fmt, ts?)` | Format timestamp with strftime pattern |
| `fs` | `read(path)` | Read file contents (sandboxed to domain folder) |
| `fs` | `exists(path)` | Check if file exists |

---

## See Also

- [Lua Scripting Guide](./LUA_SCRIPTING.md) - Core concepts and module details
- [API Reference](./API.md) - Mock server HTTP API
- [IDE Support](./IDE_SUPPORT.md) - Setting up autocomplete
