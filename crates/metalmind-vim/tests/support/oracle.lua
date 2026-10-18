-- Oráculo: aplica teclas en nvim y escribe cursor, modo, el último mensaje y
-- el texto en un archivo.
-- Uso: nvim --clean --headless -n -l oracle.lua <texto> <fila> <columna> <teclas> <salida> [<filas> <scrolloff> <primera línea>]
local text, row, col, keys, out = arg[1], tonumber(arg[2]), tonumber(arg[3]), arg[4], arg[5]
local rows, scrolloff, top = tonumber(arg[6]), tonumber(arg[7]), tonumber(arg[8])

-- Las teclas van a otro nvim por nvim_input, como las manda la TUI: así
-- cuentan como tecleadas y un <C-c> interrumpe (no repite `3ihola`).
local child = vim.fn.jobstart({ vim.v.progpath, "--clean", "--headless", "--embed", "-n" }, { rpc = true })

-- Una petición que no es "fast" se atiende cuando nvim ya consumió lo tecleado.
local function exec(code, ...)
  return vim.rpcrequest(child, "nvim_exec_lua", code, { ... })
end

exec(
  [[
  local text, row, col = ...
  -- Metalmind todavía no implementa autoindent.
  vim.o.autoindent = false
  -- Metalmind implementa el `%` de vim (`findmatchlimit`), no el de matchit,
  -- que nvim carga por defecto.
  for _, mode in ipairs({ "n", "x", "o" }) do
    vim.keymap.del(mode, "%")
  end
  -- La búsqueda de Metalmind usa estos defaults, como casi toda config de vim.
  vim.o.ignorecase = true
  vim.o.smartcase = true
  -- Cargar el texto sin dejarlo en el historial de undo.
  vim.o.undolevels = -1
  vim.api.nvim_buf_set_lines(0, 0, -1, true, vim.split(text, "\n", { plain = true }))
  vim.o.undolevels = 1000
  vim.api.nvim_win_set_cursor(0, { row, col })
]],
  text,
  row,
  col
)

-- Una ventana de `rows` filas: `lines` cuenta también la barra de estado y
-- la línea de comandos.
if rows then
  local height = exec(
    [[
    local rows, scrolloff, top, row, col = ...
    vim.o.lines = rows + 2
    vim.o.scrolloff = scrolloff
    vim.fn.winrestview({ topline = top, lnum = row, col = col })
    return vim.fn.winheight(0)
  ]],
    rows,
    scrolloff,
    top,
    row,
    col
  )
  assert(height == rows, "la ventana tiene " .. height .. " filas, no " .. rows)
end

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

-- En Visual, getpos("v") es el otro extremo de la selección. El último
-- mensaje es el de la línea de comandos: un error o un aviso como el de `:g`.
local state = exec([[
  local pos = vim.api.nvim_win_get_cursor(0)
  local anchor = vim.fn.getpos("v")
  local mode = vim.api.nvim_get_mode().mode
  local messages = vim.split(vim.fn.execute("messages"), "\n", { trimempty = true })
  return {
    pos[1], pos[2], mode, anchor[2], anchor[3] - 1, vim.fn.line("w0"),
    messages[#messages] or "", vim.api.nvim_buf_get_lines(0, 0, -1, true),
  }
]])
local file = assert(io.open(out, "w"))
file:write(string.format("%d %d %s %d %d %d\n", state[1], state[2], state[3], state[4], state[5], state[6]))
file:write(state[7] .. "\n")
file:write(table.concat(state[8], "\n"))
file:close()

vim.rpcnotify(child, "nvim_command", "qa!")
vim.fn.jobwait({ child }, 1000)
