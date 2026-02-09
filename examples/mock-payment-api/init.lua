-- mock-payment-api: Mock a Stripe-like payment gateway for e-commerce
-- integration tests.
--
-- Try:
--   curl -X POST -H "Host: mock-payment-api" -H "Content-Type: application/json" \
--     -d '{"amount":2000,"currency":"usd"}' localhost:3000/v1/charges
--   curl -H "Host: mock-payment-api" localhost:3000/v1/charges

local json = require("json")
local state = require("state")
local uuid = require("uuid")
local time = require("time")

local function json_response(status, data)
    return {
        status = status,
        headers = { ["Content-Type"] = "application/json" },
        body = json.encode(data),
    }
end

local function get_charges()
    return state.get("charges") or {}
end

---@param request Request
---@return Response
function handle(request)
    local charges = get_charges()

    -- POST /v1/charges — create a charge
    if request.method == "POST" and request.path == "/v1/charges" then
        local data = json.decode(request.body)
        local id = "ch_" .. uuid.v4()
        local charge = {
            id = id,
            amount = data.amount,
            currency = data.currency or "usd",
            status = "succeeded",
            created_at = time.iso8601(),
        }
        charges[id] = charge
        state.set("charges", charges)
        return json_response(201, charge)
    end

    -- GET /v1/charges — list all charges
    if request.method == "GET" and request.path == "/v1/charges" then
        local list = {}
        for _, charge in pairs(charges) do
            table.insert(list, charge)
        end
        return json_response(200, { charges = list, total = #list })
    end

    -- GET /v1/charges/:id — retrieve a charge
    local charge_id = request.path:match("^/v1/charges/([^/]+)$")
    if request.method == "GET" and charge_id then
        if charges[charge_id] then
            return json_response(200, charges[charge_id])
        end
        return json_response(404, { error = "Charge not found" })
    end

    -- POST /v1/refunds — refund a charge
    if request.method == "POST" and request.path == "/v1/refunds" then
        local data = json.decode(request.body)
        local cid = data.charge
        if not cid or not charges[cid] then
            return json_response(404, { error = "Charge not found" })
        end
        charges[cid].status = "refunded"
        state.set("charges", charges)
        return json_response(200, {
            id = "re_" .. uuid.v4(),
            charge = cid,
            status = "succeeded",
            created_at = time.iso8601(),
        })
    end

    return json_response(404, { error = "Not found" })
end
