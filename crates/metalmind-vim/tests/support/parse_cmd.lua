-- Escribe, por cada línea de `:`, el comando que entiende nvim con su `!`.
-- Uso: nvim --clean --headless -n -l parse_cmd.lua <salida> <línea>...
local out = assert(io.open(arg[1], "w"))
for i = 2, #arg do
  local ok, parsed = pcall(vim.api.nvim_parse_cmd, arg[i], {})
  out:write((ok and (parsed.cmd .. (parsed.bang and "!" or "")) or "error") .. "\n")
end
out:close()
