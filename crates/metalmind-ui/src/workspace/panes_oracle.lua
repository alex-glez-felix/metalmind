-- Oráculo de paneles: abre <archivo> en <carpeta>, teclea <teclas> en nvim y
-- escribe el modo, el último error, el último mensaje, lo que muestra
-- 'showmode', los buffers con cambios y, por ventana, archivo, si tiene el
-- foco y cursor. Los mensajes, solo con <mensajes> = 1: la UI que los recibe
-- cambia lo demás (los errores seguidos piden confirmar).
-- Uso: nvim --clean --headless -n -l panes_oracle.lua <carpeta> <archivo> <teclas> <salida> <mensajes>
local dir, file, keys, out, messages = arg[1], arg[2], arg[3], arg[4], arg[5] == "1"

-- Las teclas van a otro nvim por nvim_input, como las manda la TUI, igual
-- que en el oráculo de metalmind-vim: así un <C-c> interrumpe de verdad.
local child = vim.fn.jobstart({ vim.v.progpath, "--clean", "--headless", "--embed", "-n" }, { rpc = true, cwd = dir })

local function exec(code, ...)
  return vim.rpcrequest(child, "nvim_exec_lua", code, { ... })
end

-- Los mensajes se leen como los recibe una UI con `ext_messages`: los de
-- búsqueda y 'showmode' no quedan en `:messages`. Como Metalmind, avisa al
-- abrir un archivo (sin `F`) y al dar la vuelta una búsqueda, sin contar
-- coincidencias (`S`).
exec([[
  _G.last_message, _G.showmode = "", ""
  if not ... then
    return
  end
  vim.o.shortmess = vim.o.shortmess:gsub("F", "") .. "S"
  local function text(content)
    local chunks = {}
    for _, chunk in ipairs(content) do
      chunks[#chunks + 1] = chunk[2]
    end
    return (table.concat(chunks):gsub("\n", "\\n"))
  end
  vim.ui_attach(vim.api.nvim_create_namespace("oraculo"), { ext_messages = true }, function(event, ...)
    local args = { ... }
    if event == "msg_show" and args[1] ~= "search_cmd" then
      _G.last_message = text(args[2])
    elseif event == "msg_showmode" then
      _G.showmode = text(args[1])
    end
  end)
]], messages)

-- Con `hidden`, el default de nvim, como Metalmind.
exec("vim.o.autoindent = false; vim.cmd.edit(...)", file)

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
  local modified = {}
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].modified then
      modified[#modified + 1] = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(buf), ":t")
    end
  end
  table.sort(modified)
  local lines = {
    vim.api.nvim_get_mode().mode, vim.v.errmsg, _G.last_message, _G.showmode, table.concat(modified, " "),
  }
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
