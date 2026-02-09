require("conform").formatters_by_ft.rust = { "rustfmt" }
require("conform").formatters_by_ft.toml = { "taplo" }
vim.lsp.enable({ "rust_analyzer", "ty" })

-- License header helpers
local function add_license_header(file)
  file = file or vim.api.nvim_buf_get_name(0)
  if file == "" then return end
  vim.cmd("!scripts/reuse-header " .. vim.fn.shellescape(file))
  vim.cmd("edit") -- reload buffer
end

-- :ReuseLicense command
vim.api.nvim_create_user_command("ReuseLicense", function()
  add_license_header()
end, { desc = "Add AGPL-3.0 license header to current file" })
