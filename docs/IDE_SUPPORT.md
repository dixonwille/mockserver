# IDE Support

Mockserver generates [LuaLS](https://github.com/LuaLS/lua-language-server) type definitions that provide:

- **Autocomplete** for request/response objects and built-in modules
- **Type checking** to catch errors before running
- **Inline documentation** on hover
- **Go to definition** for module functions

## Generated Files

```
.mockserver/mocks/
  _types/              One .lua file per module (json, log, delay, state, uuid, time, fs)
    types.lua          Request, Response, and handle() definitions
  .luarc.json          LuaLS workspace configuration
```

Run `mockserver init` to generate these files. Run `mockserver init --force` to regenerate after upgrading mockserver (preserves your mock scripts).

## IDE Setup

### Visual Studio Code

1. Install the [Lua extension by sumneko](https://marketplace.visualstudio.com/items?itemName=sumneko.lua)
2. Open the folder containing `.mockserver/mocks/`
3. The `.luarc.json` is automatically detected

### Neovim

Configure `lua_ls` via [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
require("lspconfig").lua_ls.setup {}
```

LuaLS automatically detects `.luarc.json` in the workspace.

### JetBrains IDEs

1. Install the [EmmyLua plugin](https://plugins.jetbrains.com/plugin/9768-emmylua)
2. Open the folder containing `.mockserver/mocks/`
3. The plugin reads EmmyLua annotations from `_types/`

## Version Control

**Commit `_types/` and `.luarc.json` to version control.** This ensures all team members get IDE support immediately without running `mockserver init`.

Recommended `.gitignore`:

```gitignore
.mockserver/data/
```

Do not ignore `.mockserver/mocks/` -- it contains your mocks and type definitions.

## Related Documentation

- [Lua Scripting](./LUA_SCRIPTING.md) -- Module APIs and the handle() contract
- [CLI](./CLI.md) -- The `init` and `init --force` commands
