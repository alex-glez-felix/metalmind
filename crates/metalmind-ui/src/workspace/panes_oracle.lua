-- Oráculo de paneles: abre <archivo> en <carpeta>, teclea <teclas> en nvim y
-- escribe el modo y, por ventana, archivo, si tiene el foco y cursor.
-- Uso: nvim --clean --headless -n -l panes_oracle.lua <carpeta> <archivo> <teclas> <salida>
local dir, file, keys, out = arg[1], arg[2], arg[3], arg[4]

-- Las teclas van a otro nvim por nvim_input, como las manda la TUI, igual
-- que en el oráculo de metalmind-vim: así un <C-c> interrumpe de verdad.
local child = vim.fn.jobstart({ vim.v.progpath, "--clean", "--headless", "--embed", "-n" }, { rpc = true, cwd = dir })

local function exec(code, ...)
  return vim.rpcrequest(child, "nvim_exec_lua", code, { ... })
end

-- Metalmind es vim sin `hidden` (03 §8): `:e` se niega si se perderían cambios.
exec("vim.o.autoindent = false; vim.o.hidden = false; vim.cmd.edit(...)", file)

local function type_keys(typed)
  vim.rpcrequest(child, "nvim_input", typed)
  exec("return 0")
end

-- Al llegar, un <C-c> descarta lo que siga sin procesar: va en su propio tramo.
local rest = keys
while rest ~= "" do
  local start, stop = rest:find("<C%-c>")
  if not start then
    type_keys(rest)
    break
  end
  if start > 1 then
    type_keys(rest:sub(1, start - 1))
  end
  type_keys("<C-c>")
  rest = rest:sub(stop + 1)
end

local lines = exec([[
  local lines = { vim.api.nvim_get_mode().mode }
  local current = vim.api.nvim_get_current_win()
  for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
    local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(vim.api.nvim_win_get_buf(win)), ":t")
    local cursor = vim.api.nvim_win_get_cursor(win)
    lines[#lines + 1] = string.format("%s %d %d %d", name, win == current and 1 or 0, cursor[1], cursor[2])
  end
  return lines
]])
local output = assert(io.open(out, "w"))
output:write(table.concat(lines, "\n"))
output:close()

vim.rpcnotify(child, "nvim_command", "qa!")
vim.fn.jobwait({ child }, 1000)
