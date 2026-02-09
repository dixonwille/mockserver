-- mock-weather-service: Mock a weather API so your dashboard tests get
-- predictable data without network calls.
--
-- Try:
--   curl -H "Host: mock-weather-service" "localhost:3000/v1/current?city=london"
--   curl -H "Host: mock-weather-service" "localhost:3000/v1/forecast?city=london&days=3"

local json = require("json")
local fs = require("fs")
local log = require("log")

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
    -- GET /v1/current?city=<name>
    if request.method == "GET" and request.path == "/v1/current" then
        local city = request.query.city
        if not city then
            return json_response(400, { error = "Missing 'city' query parameter" })
        end

        local data = json.decode(fs.read("fixtures/cities.json"))
        local key = city:lower()
        log.info("Weather lookup for city: " .. key)

        if data[key] then
            return json_response(200, data[key])
        end
        return json_response(404, { error = "City not found: " .. city })
    end

    -- GET /v1/forecast?city=<name>&days=<n>
    if request.method == "GET" and request.path == "/v1/forecast" then
        local city = request.query.city
        if not city then
            return json_response(400, { error = "Missing 'city' query parameter" })
        end

        local days = tonumber(request.query.days) or 3
        if days > 7 then days = 7 end
        if days < 1 then days = 1 end

        local data = json.decode(fs.read("fixtures/forecasts.json"))
        local key = city:lower()
        log.info("Forecast lookup for city: " .. key .. " days: " .. days)

        if not data[key] then
            return json_response(404, { error = "City not found: " .. city })
        end

        local forecast = {}
        for i = 1, math.min(days, #data[key]) do
            table.insert(forecast, data[key][i])
        end
        return json_response(200, { city = key, days = days, forecast = forecast })
    end

    return json_response(404, { error = "Not found" })
end
