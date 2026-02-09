-- mock-github-graphql: Mock GitHub's GraphQL API for a CI dashboard that
-- queries PRs and issues.
--
-- Try:
--   curl -X POST -H "Host: mock-github-graphql" -H "Content-Type: application/json" \
--     -d '{"query":"{ viewer { login } }"}' localhost:3000/graphql

local json = require("json")
local log = require("log")

local function gql_response(data)
    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({ data = data }),
    }
end

local function gql_error(message)
    return {
        status = 200,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode({ errors = { { message = message } } }),
    }
end

---@param request Request
---@return Response
function handle(request)
    if request.method ~= "POST" then
        return {
            status = 405,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ error = "Method not allowed" }),
        }
    end

    if request.path ~= "/graphql" then
        return {
            status = 404,
            headers = { ["Content-Type"] = "application/json" },
            body = json.encode({ error = "Not found" }),
        }
    end

    local body = json.decode(request.body)
    local query = body.query or ""
    log.info("GraphQL query received: " .. query:sub(1, 80))

    -- Viewer query
    if query:find("viewer") then
        return gql_response({
            viewer = {
                login = "octocat",
                name = "The Octocat",
                avatarUrl = "https://avatars.example.com/octocat",
            },
        })
    end

    -- Pull requests query
    if query:find("pullRequests") then
        return gql_response({
            repository = {
                pullRequests = {
                    nodes = {
                        {
                            number = 42,
                            title = "Add new feature",
                            state = "OPEN",
                            author = { login = "octocat" },
                        },
                        {
                            number = 41,
                            title = "Fix bug in parser",
                            state = "MERGED",
                            author = { login = "contributor" },
                        },
                    },
                },
            },
        })
    end

    -- Issues query
    if query:find("issues") then
        return gql_response({
            repository = {
                issues = {
                    nodes = {
                        {
                            number = 10,
                            title = "Bug: login fails",
                            state = "OPEN",
                            labels = { nodes = { { name = "bug" } } },
                        },
                        {
                            number = 9,
                            title = "Feature request: dark mode",
                            state = "OPEN",
                            labels = { nodes = { { name = "enhancement" } } },
                        },
                    },
                },
            },
        })
    end

    -- createIssue mutation
    if query:find("createIssue") then
        local vars = body.variables or {}
        return gql_response({
            createIssue = {
                issue = {
                    number = 99,
                    title = vars.title or "New issue",
                    state = "OPEN",
                },
            },
        })
    end

    return gql_error("Unknown query")
end
